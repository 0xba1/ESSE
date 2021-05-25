#[rustfmt::skip]
pub(super) const SESSION_VERSIONS: [&str; 3] = [
  "CREATE TABLE IF NOT EXISTS sessions(
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    fid INTEGER NOT NULL,
    gid TEXT NOT NULL,
    addr TEXT NOT NULL,
    s_type INTEGER NOT NULL,
    name TEXT NOT NULL,
    is_top INTEGER NOT NULL,
    last_datetime INTEGER,
    last_content TEXT,
    last_readed INTEGER);",
  "INSERT INTO sessions (fid, gid, addr, s_type, name, is_top, last_datetime, last_content, last_readed) VALUES (0, '', '', 4, '', 0, 0, '', 1);", // Assistant.
  "INSERT INTO sessions (fid, gid, addr, s_type, name, is_top, last_datetime, last_content, last_readed) VALUES (0, '', '', 2, '', 0, 0, '', 1);", // File.
];
