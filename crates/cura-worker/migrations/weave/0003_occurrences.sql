-- The DIP row is the fact, not the occurrence. A fact is keyed by
-- (weave, pattern, intent) and accumulates: how many times it was
-- seen, when first, when last. Duplicate writes increment, they do
-- not insert. The old schema (INSERT OR IGNORE + COUNT(*)) could not
-- distinguish "seen once" from "seen a thousand times".

ALTER TABLE weave ADD COLUMN occurrences INTEGER NOT NULL DEFAULT 1;
ALTER TABLE weave ADD COLUMN first_seen  INTEGER NOT NULL DEFAULT 0;
ALTER TABLE weave ADD COLUMN last_seen   INTEGER NOT NULL DEFAULT 0;

UPDATE weave
SET first_seen = observed_at,
    last_seen  = observed_at
WHERE first_seen = 0;

-- A fact can be reached from more than one language. source_lang on
-- the weave row is the first writer; the full set lives here.
CREATE TABLE IF NOT EXISTS weave_lang (
  weave       TEXT NOT NULL,
  pattern     TEXT NOT NULL,
  intent      TEXT NOT NULL,
  source_lang TEXT NOT NULL,
  first_seen  INTEGER NOT NULL,
  last_seen   INTEGER NOT NULL,
  PRIMARY KEY (weave, pattern, intent, source_lang)
);
CREATE INDEX IF NOT EXISTS weave_lang_lang_idx   ON weave_lang (source_lang);
CREATE INDEX IF NOT EXISTS weave_lang_intent_idx ON weave_lang (intent);
CREATE INDEX IF NOT EXISTS weave_lang_pattern_idx ON weave_lang (pattern);

-- Rebuild the views against SUM(occurrences), not COUNT(*).
DROP VIEW IF EXISTS v_ways_to_intent;
DROP VIEW IF EXISTS v_intent_frontier;
DROP VIEW IF EXISTS v_signature_patterns;
DROP VIEW IF EXISTS v_language_by_intent;

CREATE VIEW v_ways_to_intent AS
SELECT intent,
       pattern,
       weave,
       SUM(occurrences) AS occurrences,
       MIN(first_seen)  AS first_seen,
       MAX(last_seen)   AS last_seen
FROM weave
GROUP BY intent, pattern, weave;

CREATE VIEW v_intent_frontier AS
SELECT w.intent,
       COUNT(DISTINCT w.pattern) AS patterns,
       COUNT(DISTINCT w.weave)   AS weaves,
       (SELECT COUNT(DISTINCT source_lang)
          FROM weave_lang wl
         WHERE wl.intent = w.intent) AS languages,
       SUM(w.occurrences) AS total_occurrences,
       MAX(CASE WHEN w.occurrences = 1 THEN 1 ELSE 0 END) AS has_singletons
FROM weave w
GROUP BY w.intent;

CREATE VIEW v_signature_patterns AS
SELECT pattern,
       source_lang,
       GROUP_CONCAT(DISTINCT weave) AS weaves
FROM weave_lang
GROUP BY pattern
HAVING COUNT(DISTINCT source_lang) = 1;

CREATE VIEW v_language_by_intent AS
SELECT source_lang,
       intent,
       COUNT(DISTINCT pattern) AS patterns
FROM weave_lang
GROUP BY source_lang, intent;
