-- cura-di
-- One row per distinct intent value. Binary is the intent's form
-- on the substrate. first_seen / last_seen are wall-clock at write.
CREATE TABLE IF NOT EXISTS intent (
  value       TEXT PRIMARY KEY,
  binary      TEXT NOT NULL DEFAULT '',
  first_seen  INTEGER NOT NULL,
  last_seen   INTEGER NOT NULL
);
