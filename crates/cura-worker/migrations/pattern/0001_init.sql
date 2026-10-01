-- cura-dp
-- One row per distinct pattern. A pattern points at the intent it
-- reaches. The same intent can be reached by many patterns; the same
-- pattern always reaches exactly one intent.
CREATE TABLE IF NOT EXISTS pattern (
  pattern     TEXT PRIMARY KEY,
  intent      TEXT NOT NULL,
  first_seen  INTEGER NOT NULL,
  last_seen   INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS pattern_intent_idx ON pattern (intent);
