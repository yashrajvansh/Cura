-- Add source_lang to DI, for symmetry with DP and DIP. No views here —
-- the two-axis map views are DIP-local; see migrations/weave/0002.
ALTER TABLE intent ADD COLUMN source_lang TEXT NOT NULL DEFAULT '';
CREATE INDEX IF NOT EXISTS intent_source_lang_idx ON intent (source_lang);
