import 'package:shared_preferences/shared_preferences.dart';

import 'package:esse/account.dart';

const LOGINED_CACHE_NAME = 'logined';

/// get all auto-logined account. first one is main.
Future<List<Account>> getLogined() async {
  List<Account> accounts = [];
  SharedPreferences prefs = await SharedPreferences.getInstance();
  final ids = prefs.getStringList(LOGINED_CACHE_NAME);
  if (ids != null) {
    ids.forEach((id) {
      final fields = prefs.getStringList(id);
      if (fields != null && fields.length == 5) {
        accounts.add(Account(
          fields[0], // gid
          fields[1], // name
          fields[2], // avatar
          false,
        ));
      } else {
        prefs.remove(id);
      }
    });
  }

  return accounts;
}

initLogined(String gid, List<Account> accounts) async {
  SharedPreferences prefs = await SharedPreferences.getInstance();
  final ids = prefs.getStringList(LOGINED_CACHE_NAME);
  if (ids != null) {
    ids.forEach((id) {
      prefs.remove(id);
    });
  }

  List<String> newIds = [gid];
  accounts.forEach((account) {
    final List<String> fields = [
      account.gid,
      account.name,
      account.encodeAvatar(),
    ];

    if (account.gid != gid) {
      newIds.add(account.gid);
    }

    prefs.setStringList(account.gid, fields);
  });

  prefs.setStringList(LOGINED_CACHE_NAME, newIds);
}

/// update auto-logined account.
updateLogined(Account account) async {
  SharedPreferences prefs = await SharedPreferences.getInstance();
  List<String>? ids = prefs.getStringList(LOGINED_CACHE_NAME);
  if (ids == null) {
    ids = [];
  }


  if (!ids.contains(account.gid)) {
    ids.add(account.gid);
    prefs.setStringList(LOGINED_CACHE_NAME, ids);
  }

  final List<String> fields = [
    account.gid,
    account.name,
    account.encodeAvatar(),
  ];

  prefs.setStringList(account.gid, fields);
}

/// change main logined account.
mainLogined(String gid) async {
  SharedPreferences prefs = await SharedPreferences.getInstance();
  List<String>? ids = prefs.getStringList(LOGINED_CACHE_NAME);
  if (ids == null) {
    ids = [];
  }

  ids.remove(gid);
  ids.insert(0, gid);
  prefs.setStringList(LOGINED_CACHE_NAME, ids);
}

/// remove auto-login accounts.
removeLogined(String gid) async {
  SharedPreferences prefs = await SharedPreferences.getInstance();
  prefs.remove(gid);
  List<String>? ids = prefs.getStringList(LOGINED_CACHE_NAME);
  if (ids == null) {
    ids = [];
  }

  if (ids.contains(gid)) {
    ids.remove(gid);
    prefs.setStringList(LOGINED_CACHE_NAME, ids);
  }
}

/// when logout clear all
clearLogined() async {
  SharedPreferences prefs = await SharedPreferences.getInstance();
  List<String>? ids = prefs.getStringList(LOGINED_CACHE_NAME);
  if (ids == null) {
    ids = [];
  }

  ids.forEach((id) {
    prefs.remove(id);
  });
  prefs.remove(LOGINED_CACHE_NAME);
}
