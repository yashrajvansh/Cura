-- Add source_lang to the DIP table. The `source` column carries a
-- composite path string ("rust:path:fn:name", "sheath:path:op"); this
-- new column carries just the language token ("rust", "sheath",
-- "jvm-bytecode", ...) so cross-language queries don't need regex.
ALTER TABLE weave ADD COLUMN source_lang TEXT NOT NULL DEFAULT '';
CREATE INDEX IF NOT EXISTS weave_source_lang_idx ON weave (source_lang);

-- Two-axis map views. Probability (count) and possibility (presence)
-- stay separate columns; no view collapses them.
CREATE VIEW IF NOT EXISTS v_ways_to_intent AS
SELECT intent,
       pattern,
       weave,
       COUNT(*)             AS occurrences,
       MIN(observed_at)     AS first_seen,
       MAX(observed_at)     AS last_seen
FROM weave
GROUP BY intent, pattern, weave;

CREATE VIEW IF NOT EXISTS v_intent_frontier AS
SELECT intent,
       COUNT(DISTINCT pattern)                             AS patterns,
       COUNT(DISTINCT weave)                               AS weaves,
       COUNT(DISTINCT source_lang)                         AS languages,
       SUM(occurrences)                                    AS total_occurrences,
       MAX(CASE WHEN occurrences = 1 THEN 1 ELSE 0 END)    AS has_singletons
FROM v_ways_to_intent
GROUP BY intent;

CREATE VIEW IF NOT EXISTS v_signature_patterns AS
SELECT pattern,
       source_lang,
       GROUP_CONCAT(DISTINCT weave) AS weaves
FROM weave
GROUP BY pattern
HAVING COUNT(DISTINCT source_lang) = 1;

CREATE VIEW IF NOT EXISTS v_language_by_intent AS
SELECT source_lang,
       intent,
       COUNT(DISTINCT pattern) AS patterns
FROM weave
GROUP BY source_lang, intent;
