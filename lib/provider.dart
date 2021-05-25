import 'dart:async';
import "dart:collection";
import 'dart:typed_data';

import 'package:flutter/material.dart';

import 'package:esse/account.dart';
import 'package:esse/utils/logined_cache.dart';
import 'package:esse/widgets/default_core_show.dart';
import 'package:esse/widgets/default_home_show.dart';
import 'package:esse/global.dart';
import 'package:esse/rpc.dart';
import 'package:esse/session.dart';

const DEFAULT_ONLINE_INIT = 8;
const DEFAULT_ONLINE_DELAY = 5;

class AccountProvider extends ChangeNotifier {
  Map<String, Account> accounts = {}; // account's gid and account.
  String activedAccountId; // actived account gid.
  Account get activedAccount => this.accounts[activedAccountId];

  /// home sessions. sorded by last_time.
  Map<int, Session> sessions = {};
  List<int> topKeys = [];
  List<int> orderKeys = [];

  /// current user's did.
  String get id => this.activedAccount.id;

  String homeShowTitle = '';
  Widget defaultListShow = DefaultHomeShow();
  Widget currentListShow = null;
  Widget coreShowWidget = DefaultCoreShow();
  bool systemAppFriendAddNew = false;

  Widget get homeShowWidget => this.currentListShow ?? this.defaultListShow;

  void orderSessions(int id) {
    if (this.orderKeys.length == 0 || this.orderKeys[0] != id) {
      this.orderKeys.remove(id);
      this.orderKeys.insert(0, id);
    }
  }

  AccountProvider() {
    // rpc notice when account not actived.
    rpc.addNotice(_accountNotice, _newRequestNotice);

    // rpc
    rpc.addListener('account-system-info', _systemInfo, false);
    rpc.addListener('account-update', _accountUpdate, false);
    rpc.addListener('account-login', _accountLogin, false);

    rpc.addListener('session-list', _sessionList, false);
    rpc.addListener('session-last', _sessionLast, true);
    rpc.addListener('session-create', _sessionCreate, true);
    rpc.addListener('session-update', _sessionUpdate, false);
    rpc.addListener('session-delete', _sessionDelete, false);

    systemInfo();
  }

  /// when security load accounts from rpc.
  initAccounts(Map accounts) {
    this.accounts = accounts;
    initLogined(this.accounts.values.toList());
  }

  /// when security load accounts from cache.
  autoAccounts(String gid, Map accounts) {
    Global.changeGid(gid);
    this.activedAccountId = gid;
    this.accounts = accounts;

    this.activedAccount.online = true;
    rpc.send('account-login', [gid, this.activedAccount.lock]);
    rpc.send('session-list', []);

    new Future.delayed(Duration(seconds: DEFAULT_ONLINE_INIT),
      () => rpc.send('account-online', [gid]));

    this.accounts.forEach((k, v) {
      if (k != gid && v.online) {
        rpc.send('account-login', [v.gid, v.lock]);
        new Future.delayed(Duration(seconds: DEFAULT_ONLINE_INIT),
            () => rpc.send('account-online', [v.gid]));
      }
    });
  }

  /// when security add account.
  addAccount(Account account) {
    Global.changeGid(account.gid);
    this.activedAccountId = account.gid;
    this.accounts[account.gid] = account;

    rpc.send('account-login', [account.gid, account.lock]);
    rpc.send('session-list', []);

    new Future.delayed(Duration(seconds: DEFAULT_ONLINE_DELAY),
        () => rpc.send('account-online', [account.gid]));
    updateLogined(account);
  }

  updateActivedAccount(String gid) {
    Global.changeGid(gid);
    this.clearActivedAccount();
    this.activedAccountId = gid;
    this.activedAccount.hasNew = false;
    this.coreShowWidget = DefaultCoreShow();
    this.currentListShow = null;

    // load sessions.
    this.sessions.clear();
    this.orderKeys.clear();
    rpc.send('session-list', []);

    if (!this.activedAccount.online) {
      this.activedAccount.online = true;
      rpc.send('account-login', [gid, this.activedAccount.lock]);
      new Future.delayed(Duration(seconds: DEFAULT_ONLINE_DELAY),
          () => rpc.send('account-online', [gid]));
    }

    mainLogined(gid);
    notifyListeners();
  }

  logout() {
    this.accounts.clear();
    this.clearActivedAccount();
    this.currentListShow = null;
    this.sessions.clear();
    this.orderKeys.clear();
    this.topKeys.clear();

    rpc.send('account-logout', []);
    clearLogined();
  }

  onlineAccount(String gid, String lock) {
    this.accounts[gid].online = true;
    updateLogined(this.accounts[gid]);

    rpc.send('account-login', [gid, lock]);

    new Future.delayed(Duration(seconds: DEFAULT_ONLINE_DELAY),
        () => rpc.send('account-online', [gid]));
    notifyListeners();
  }

  offlineAccount(String gid) {
    this.accounts[gid].online = false;
    updateLogined(this.accounts[gid]);

    if (gid == this.activedAccountId) {
      this.clearActivedAccount();
    }
    rpc.send('account-offline', [gid]);

    notifyListeners();
  }

  updateToHome() {
    this.homeShowTitle = '';
    this.currentListShow = null;
    notifyListeners();
  }

  updateActivedApp([Widget coreWidget, String title, Widget homeWidget]) {
    if (homeWidget != null && title != null) {
      this.homeShowTitle = title;
      this.currentListShow = homeWidget;
    }
    if (coreWidget != null) {
      this.coreShowWidget = coreWidget;
    }
    this.systemAppFriendAddNew = false;
    notifyListeners();
  }

  clearActivedAccount() {
    this.topKeys.clear();
  }

  accountUpdate(String name, [Uint8List avatar]) {
    this.activedAccount.name = name;

    if (avatar != null && avatar.length > 0) {
      this.activedAccount.avatar = avatar;
      rpc.send('account-update', [name, this.activedAccount.encodeAvatar()]);
    } else {
      rpc.send('account-update', [name, '']);
    }
    updateLogined(this.activedAccount);

    notifyListeners();
  }

  accountPin(String lock) {
    this.activedAccount.lock = lock;
    updateLogined(this.activedAccount);
    notifyListeners();
  }

  systemInfo() {
    rpc.send('account-system-info', []);
  }

  // -- callback when receive rpc info. -- //
  _systemInfo(List params) {
    Global.addr = '0x' + params[0];
  }

  _accountLogin(List _params) {
    // nothing.
  }

  _accountNotice(String gid) {
    if (this.accounts.containsKey(gid)) {
      this.accounts[gid].hasNew = true;
      notifyListeners();
    }
  }

  _newRequestNotice(String gid) {
    if (this.activedAccountId == gid) {
      this.systemAppFriendAddNew = true;
      notifyListeners();
    }
  }

  _accountUpdate(List params) {
    final gid = params[0];
    this.accounts[gid].name = params[1];
    if (params[2].length > 1) {
      this.accounts[gid].updateAvatar(params[2]);
    }
    notifyListeners();
  }

  _sessionList(List params) {
    this.sessions.clear();
    this.orderKeys.clear();
    this.topKeys.clear();

    params.forEach((params) {
        final id = params[0];
        this.sessions[id] = Session.fromList(params);

        if (this.sessions[id].isTop) {
          this.topKeys.add(id);
        } else {
          this.orderKeys.add(id);
        }

    });
    notifyListeners();
  }

  _sessionCreate(List params) {
    final id = params[0];
    this.sessions[id] = Session.fromList(params);
    orderSessions(id);
    notifyListeners();
  }

  _sessionLast(List params) {
    final id = params[0];
    this.sessions[id].last(params);
    orderSessions(id);
    notifyListeners();
  }

  _sessionUpdate(List params) {
    final id = params[0];
    this.sessions[id].update(params);
    if (this.sessions[id].isTop) {
      this.topKeys.add(id);
      this.orderKeys.remove(id);
    } else {
      orderSessions(id);
      this.topKeys.remove(id);
    }
    notifyListeners();
  }

  _sessionDelete(List params) {
    final id = params[1];
    this.sessions.remove(id);
    this.orderKeys.remove(id);
    this.topKeys.remove(id);
    notifyListeners();
  }
}
