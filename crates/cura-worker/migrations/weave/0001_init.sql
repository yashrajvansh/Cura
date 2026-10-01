-- cura-dip
-- One row per distinct (weave, pattern, intent) triple. This is the
-- full fact: which rules acted on which surface to reach which intent.
-- DI and DP are deduplicated indexes over the values appearing here.
CREATE TABLE IF NOT EXISTS weave (
  weave       TEXT NOT NULL,
  pattern     TEXT NOT NULL,
  intent      TEXT NOT NULL,
  op          TEXT NOT NULL,
  witness     TEXT NOT NULL,
  observed_at INTEGER NOT NULL,
  PRIMARY KEY (weave, pattern, intent)
);
CREATE INDEX IF NOT EXISTS weave_intent_idx  ON weave (intent);
CREATE INDEX IF NOT EXISTS weave_pattern_idx ON weave (pattern);
CREATE INDEX IF NOT EXISTS weave_weave_idx   ON weave (weave);
