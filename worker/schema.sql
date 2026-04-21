CREATE TABLE IF NOT EXISTS usage (
  device_id TEXT NOT NULL,
  date      TEXT NOT NULL,
  count     INTEGER DEFAULT 0,
  PRIMARY KEY (device_id, date)
);

CREATE TABLE IF NOT EXISTS events (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  device_id   TEXT NOT NULL,
  event       TEXT NOT NULL,
  mode        TEXT,
  app_context TEXT,
  ts          TEXT NOT NULL
);
