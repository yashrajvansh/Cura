-- Symmetry with weave/0003. intent already carries first_seen/
-- last_seen (schema/cura-di.sql); only occurrences is new here. No
-- views -- the two-axis views are DIP-local; see migrations/weave/0003.
ALTER TABLE intent ADD COLUMN occurrences INTEGER NOT NULL DEFAULT 1;
