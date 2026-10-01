-- Add source_lang to DP, for symmetry with DI and DIP. No views here —
-- the two-axis map views are DIP-local; see migrations/weave/0002.
ALTER TABLE pattern ADD COLUMN source_lang TEXT NOT NULL DEFAULT '';
CREATE INDEX IF NOT EXISTS pattern_source_lang_idx ON pattern (source_lang);
