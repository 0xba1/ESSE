use std::sync::Arc;
use tdn::types::{
    group::GroupId,
    message::{NetworkType, SendMessage, SendType},
    primitive::{HandleResult, PeerId},
    rpc::{json, rpc_response, RpcError, RpcHandler, RpcParam},
};

use chat_types::MessageType;
use group_types::{Event, LayerEvent};

use crate::apps::chat::{Friend, InviteType};
use crate::layer::Online;
use crate::rpc::{session_create, session_delete, session_update_name, RpcState};
use crate::session::{Session, SessionType};
use crate::storage::{read_avatar, write_avatar};

use super::layer::{broadcast, update_session};
use super::models::{to_network_message, GroupChat, Member, Message};
use super::{add_layer, add_server_layer};

#[inline]
pub(crate) fn member_join(mgid: GroupId, member: &Member) -> RpcParam {
    rpc_response(0, "group-member-join", json!(member.to_rpc()), mgid)
}

#[inline]
pub(crate) fn member_leave(mgid: GroupId, id: i64, mid: i64) -> RpcParam {
    rpc_response(0, "group-member-leave", json!([id, mid]), mgid)
}

#[inline]
pub(crate) fn member_online(mgid: GroupId, id: i64, mid: i64, maddr: &PeerId) -> RpcParam {
    rpc_response(
        0,
        "group-member-online",
        json!([id, mid, maddr.to_hex()]),
        mgid,
    )
}

#[inline]
pub(crate) fn member_offline(mgid: GroupId, gid: i64, mid: i64) -> RpcParam {
    rpc_response(0, "group-member-offline", json!([gid, mid]), mgid)
}

#[inline]
pub(crate) fn group_name(mgid: GroupId, gid: &i64, name: &str) -> RpcParam {
    rpc_response(0, "group-name", json!([gid, name]), mgid)
}

#[inline]
pub(crate) fn message_create(mgid: GroupId, msg: &Message) -> RpcParam {
    rpc_response(0, "group-message-create", json!(msg.to_rpc()), mgid)
}

#[inline]
fn group_list(groups: Vec<GroupChat>) -> RpcParam {
    let mut results = vec![];
    for group in groups {
        results.push(group.to_rpc());
    }

    json!(results)
}

#[inline]
fn detail_list(group: GroupChat, members: Vec<Member>, messages: Vec<Message>) -> RpcParam {
    let mut member_results = vec![];
    for m in members {
        member_results.push(m.to_rpc());
    }

    let mut message_results = vec![];
    for msg in messages {
        message_results.push(msg.to_rpc());
    }

    json!([group.to_rpc(), member_results, message_results])
}

pub(crate) fn new_rpc_handler(handler: &mut RpcHandler<RpcState>) {
    handler.add_method(
        "group-list",
        |gid: GroupId, _params: Vec<RpcParam>, state: Arc<RpcState>| async move {
            let db = state.group.read().await.group_db(&gid)?;
            Ok(HandleResult::rpc(group_list(GroupChat::all(&db)?)))
        },
    );

    handler.add_method(
        "group-detail",
        |gid: GroupId, params: Vec<RpcParam>, state: Arc<RpcState>| async move {
            let id = params[0].as_i64().ok_or(RpcError::ParseError)?;
            let db = state.group.read().await.group_db(&gid)?;
            let group = GroupChat::get(&db, &id)?;
            let members = Member::list(&db, &id)?;
            let messages = Message::list(&db, &id)?;
            Ok(HandleResult::rpc(detail_list(group, members, messages)))
        },
    );

    handler.add_method(
        "group-create",
        |gid: GroupId, params: Vec<RpcParam>, state: Arc<RpcState>| async move {
            let name = params[0].as_str().ok_or(RpcError::ParseError)?.to_owned();

            let group_lock = state.group.read().await;
            let base = group_lock.base().clone();
            let addr = group_lock.addr().clone();
            let sender = group_lock.sender();
            let me = group_lock.clone_user(&gid)?;
            let db = group_lock.group_db(&gid)?;
            let s_db = group_lock.session_db(&gid)?;
            drop(group_lock);

            let mut gc = GroupChat::new(addr, name);
            let gcd = gc.g_id;
            let gheight = gc.height + 1; // add first member.

            // save db
            gc.insert(&db)?;
            let gdid = gc.id;

            let mut results = HandleResult::new();

            let mut m = Member::new(gheight, gc.id, gid, me.addr, me.name);
            m.insert(&db)?;
            let mid = m.id;
            let _ = write_avatar(&base, &gid, &gid, &me.avatar).await;

            // Add new session.
            let mut session = gc.to_session();
            session.insert(&s_db)?;
            let sid = session.id;
            tokio::spawn(async move {
                let _ = sender
                    .send(SendMessage::Rpc(0, session_create(gid, &session), true))
                    .await;
            });

            // add to rpcs.
            results.rpcs.push(json!([sid, gdid]));

            // Add frist member join.
            let mut layer_lock = state.layer.write().await;
            layer_lock.add_running(&gcd, gid, gdid, gheight)?;

            // Add online to layers.
            layer_lock
                .running_mut(&gcd)?
                .check_add_online(gid, Online::Direct(addr), gdid, mid)?;
            layer_lock
                .running_mut(&gid)?
                .check_add_online(gcd, Online::Direct(addr), sid, gdid)?;

            drop(layer_lock);

            // Update consensus.
            GroupChat::add_height(&db, gdid, gheight)?;

            // Online local group.
            results.networks.push(NetworkType::AddGroup(gcd));

            Ok(results)
        },
    );

    handler.add_method(
        "group-member-join",
        |gid: GroupId, params: Vec<RpcParam>, state: Arc<RpcState>| async move {
            let id = params[0].as_i64().ok_or(RpcError::ParseError)?;
            let fid = params[1].as_i64().ok_or(RpcError::ParseError)?;

            let group_lock = state.group.read().await;
            let base = group_lock.base().clone();
            let chat_db = group_lock.chat_db(&gid)?;
            let group_db = group_lock.group_db(&gid)?;
            let s_db = group_lock.session_db(&gid)?;
            drop(group_lock);
            let f = Friend::get(&chat_db, &fid)?;
            let g = GroupChat::get(&group_db, &id)?;
            let gcd = g.g_id;
            let mut results = HandleResult::new();

            // handle invite message
            let contact_values = InviteType::Group(gcd, g.g_addr, g.g_name).serialize();
            let (msg, nw) = crate::apps::chat::LayerEvent::from_message(
                &state.group,
                &base,
                gid,
                fid,
                MessageType::Invite,
                &contact_values,
            )
            .await?;
            let event = crate::apps::chat::LayerEvent::Message(msg.hash, nw);
            let mut layer_lock = state.layer.write().await;
            let s = crate::apps::chat::event_message(&mut layer_lock, msg.id, gid, f.addr, &event);
            drop(layer_lock);
            results.layers.push((gid, f.gid, s));
            crate::apps::chat::update_session(&s_db, &gid, &id, &msg, &mut results);

            // handle group member
            let avatar = read_avatar(&base, &gid, &f.gid).await.unwrap_or(vec![]);
            let event = Event::MemberJoin(f.gid, f.addr, f.name.clone(), avatar);

            if g.local {
                // local save.
                let new_h = state.layer.write().await.running_mut(&gcd)?.increased();

                let mut mem = Member::new(new_h, g.id, f.gid, f.addr, f.name);
                mem.insert(&group_db)?;
                results.rpcs.push(mem.to_rpc());
                GroupChat::add_height(&group_db, id, new_h)?;

                // broadcast.
                broadcast(
                    &LayerEvent::Sync(gcd, new_h, event),
                    &state.layer,
                    &gcd,
                    &mut results,
                )
                .await?;
            } else {
                // send to server.
                let data = bincode::serialize(&LayerEvent::Sync(gcd, 0, event))?;
                let msg = SendType::Event(0, g.g_addr, data);
                add_layer(&mut results, gid, msg);
            }

            Ok(results)
        },
    );

    handler.add_method(
        "group-message-create",
        |gid: GroupId, params: Vec<RpcParam>, state: Arc<RpcState>| async move {
            let id = params[0].as_i64().ok_or(RpcError::ParseError)?;
            let m_type = MessageType::from_int(params[1].as_i64().ok_or(RpcError::ParseError)?);
            let m_content = params[2].as_str().ok_or(RpcError::ParseError)?;

            let group_lock = state.group.read().await;
            let base = group_lock.base().clone();
            let db = group_lock.group_db(&gid)?;
            let s_db = group_lock.session_db(&gid)?;
            drop(group_lock);

            let group = GroupChat::get(&db, &id)?;
            let gcd = group.g_id;
            let mid = Member::get_id(&db, &id, &gid)?;

            let mut results = HandleResult::new();
            let (nmsg, datetime, raw) =
                to_network_message(&state.group, &base, &gid, m_type, m_content).await?;
            let event = Event::MessageCreate(gid, nmsg, datetime);

            if group.local {
                // local save.
                let new_h = state.layer.write().await.running_mut(&gcd)?.increased();

                let mut msg = Message::new_with_time(new_h, id, mid, true, m_type, raw, datetime);
                msg.insert(&db)?;
                results.rpcs.push(msg.to_rpc());
                GroupChat::add_height(&db, id, new_h)?;

                // UPDATE SESSION.
                update_session(&s_db, &gid, &id, &msg, &mut results);

                // broadcast.
                broadcast(
                    &LayerEvent::Sync(gcd, new_h, event),
                    &state.layer,
                    &gcd,
                    &mut results,
                )
                .await?;
            } else {
                // send to server.
                let data = bincode::serialize(&LayerEvent::Sync(gcd, 0, event))?;
                let msg = SendType::Event(0, group.g_addr, data);
                add_layer(&mut results, gid, msg);
            }

            Ok(results)
        },
    );

    handler.add_method(
        "group-name",
        |gid: GroupId, params: Vec<RpcParam>, state: Arc<RpcState>| async move {
            let id = params[0].as_i64().ok_or(RpcError::ParseError)?;
            let name = params[1].as_str().ok_or(RpcError::ParseError)?;

            let mut results = HandleResult::new();
            let group_lock = state.group.read().await;
            let db = group_lock.group_db(&gid)?;
            let s_db = group_lock.session_db(&gid)?;
            drop(group_lock);

            let g = GroupChat::get(&db, &id)?;
            let d = bincode::serialize(&LayerEvent::GroupName(g.g_id, name.to_owned()))?;

            if g.local {
                if let Ok(sid) = Session::update_name_by_id(&s_db, &id, &SessionType::Group, &name)
                {
                    results.rpcs.push(session_update_name(gid, &sid, &name));
                }

                results.rpcs.push(json!([id, name]));
                // dissolve group.
                for (mgid, maddr) in state.layer.read().await.running(&g.g_id)?.onlines() {
                    let s = SendType::Event(0, *maddr, d.clone());
                    add_server_layer(&mut results, *mgid, s);
                }
            } else {
                // leave group.
                let msg = SendType::Event(0, g.g_addr, d);
                add_layer(&mut results, gid, msg);
            }

            Ok(results)
        },
    );

    handler.add_method(
        "group-delete",
        |gid: GroupId, params: Vec<RpcParam>, state: Arc<RpcState>| async move {
            let id = params[0].as_i64().ok_or(RpcError::ParseError)?;

            let mut results = HandleResult::new();

            let group_lock = state.group.read().await;
            let db = group_lock.group_db(&gid)?;
            let s_db = group_lock.session_db(&gid)?;
            drop(group_lock);

            let g = GroupChat::delete(&db, &id)?;

            let sid = Session::delete(&s_db, &id, &SessionType::Group)?;
            results.rpcs.push(session_delete(gid, &sid));

            if g.local {
                // dissolve group.
                let d = bincode::serialize(&LayerEvent::GroupClose(g.g_id))?;
                for (mgid, maddr) in state.layer.read().await.running(&g.g_id)?.onlines() {
                    let s = SendType::Event(0, *maddr, d.clone());
                    add_server_layer(&mut results, *mgid, s);
                }
            } else {
                // leave group.
                let d = bincode::serialize(&LayerEvent::Sync(g.g_id, 0, Event::MemberLeave(gid)))?;
                let msg = SendType::Event(0, g.g_addr, d);
                add_layer(&mut results, gid, msg);
            }

            Ok(results)
        },
    );
}
