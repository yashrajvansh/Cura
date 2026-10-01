-- The intent's binary form now carries its kind and byte length so
-- the bytes are readable back without re-parsing the source string.
--
-- kind    — one of: null bool i64 u64 f64 f32 char str sym bytes expr hash
-- byte_len — number of bytes the binary represents
--
-- Both are backfilled to defaults for existing rows; run the backfill
-- command in the commit that adds this file if you want existing rows
-- to have real values.

ALTER TABLE intent ADD COLUMN binary_kind TEXT NOT NULL DEFAULT '';
ALTER TABLE intent ADD COLUMN binary_len  INTEGER NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS intent_kind_idx ON intent (binary_kind);

-- Rebuild the frontier view so it counts kinds.
DROP VIEW IF EXISTS v_intent_kinds;
CREATE VIEW v_intent_kinds AS
SELECT binary_kind,
       COUNT(*) AS intents
FROM intent
GROUP BY binary_kind;
