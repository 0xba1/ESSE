use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tdn::types::{
    group::GroupId,
    primitive::{Peer, PeerId, Result},
};
use tdn_did::Keypair;
use tdn_storage::local::DStorage;

use crate::apps::device::Device;
use crate::migrate::CONSENSUS_DB;

pub(crate) struct RunningAccount {
    /// secret keypair.
    pub keypair: Keypair,
    /// device's name.
    pub device_name: String,
    /// device's info.
    pub device_info: String,
    /// distribute connected devices.
    pub distributes: HashMap<PeerId, (Peer, i64, bool)>,
    /// uptime
    pub uptime: u32,
}

impl RunningAccount {
    pub fn init(keypair: Keypair, base: &PathBuf, key: &str, gid: &GroupId) -> Result<Self> {
        let mut db_path = base.clone();
        db_path.push(gid.to_hex());
        db_path.push(CONSENSUS_DB);
        let db = DStorage::open(db_path, key)?;
        let distributes = Device::distributes(&db)?;
        let (device_name, device_info) = Device::device_info(&db)?;
        db.close()?;

        let start = SystemTime::now();
        let uptime = start
            .duration_since(UNIX_EPOCH)
            .map(|s| s.as_secs())
            .unwrap_or(0) as u32; // safe for all life.

        Ok(Self {
            keypair,
            distributes,
            device_name,
            device_info,
            uptime,
        })
    }

    pub fn add_online(&mut self, addr: &PeerId) -> Result<i64> {
        if let Some(v) = self.distributes.get_mut(addr) {
            v.2 = true;
            Ok(v.1)
        } else {
            Err(anyhow!("device missing"))
        }
    }

    pub fn offline(&mut self, addr: &PeerId) -> Result<i64> {
        if let Some(v) = self.distributes.get_mut(addr) {
            v.2 = false;
            Ok(v.1)
        } else {
            Err(anyhow!("device missing"))
        }
    }
}
