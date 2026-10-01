-- Symmetry with weave/0003. pattern already carries first_seen/
-- last_seen (schema/cura-dp.sql); only occurrences is new here. No
-- views -- the two-axis views are DIP-local; see migrations/weave/0003.
ALTER TABLE pattern ADD COLUMN occurrences INTEGER NOT NULL DEFAULT 1;
