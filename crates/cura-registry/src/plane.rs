//! The DI/DP/DIP write path.

use cura_protocol::{Op, Sheathed};
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaneRow {
    pub intent: String,
    pub binary: String,
    pub binary_kind: String,
    pub binary_len: usize,
    pub pattern: String,
    pub weave: String,
    pub op: Op,
    pub witness: String,
    pub source: String,
    pub source_lang: String,
    pub observed_at_ms: i64,
}

impl PlaneRow {
    pub fn from_sheathed(s: &Sheathed) -> Self {
        Self::from_sheathed_with_lang(s, "")
    }

    pub fn from_sheathed_with_lang(s: &Sheathed, source_lang: &str) -> Self {
        let intent = s.triple.output.0.clone();

        let pattern = if s.triple.input.0.is_empty() {
            intent.clone()
        } else {
            format!("{}_{}", s.triple.input.0.replace(' ', "_"), intent)
        };

        let weave = format!("{},=", s.triple.logic.0);
        let encoded = crate::binary::encode(&intent);
        let binary = encoded.binary_string();
        let binary_kind = encoded.kind_str().to_string();
        let binary_len = encoded.byte_len();
        let source = String::from_utf8_lossy(&s.witness).into_owned();

        PlaneRow {
            intent,
            binary,
            binary_kind,
            binary_len,
            pattern,
            weave,
            op: s.op,
            witness: source.clone(),
            source,
            source_lang: source_lang.to_string(),
            observed_at_ms: now_ms(),
        }
    }

    pub fn op_str(&self) -> &'static str {
        self.op.as_str()
    }
}

pub use crate::binary::{encode as encode_intent, Encoded, Kind};

/// The machine representation of an intent. Convenience wrapper that
/// returns just the space-separated 8-bit groups.
pub fn binary_of_str(s: &str) -> String {
    encode_intent(s).binary_string()
}

/// The kind tag for an intent string.
pub fn kind_of_str(s: &str) -> &'static str {
    encode_intent(s).kind_str()
}

/// The byte length of the machine representation.
pub fn byte_len_of_str(s: &str) -> usize {
    encode_intent(s).byte_len()
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[derive(Debug, thiserror::Error)]
pub enum SinkError {
    #[error("io: {0}")]
    Io(String),
    #[error("invalid row: {0}")]
    Invalid(String),
}

/// A sink writes PlaneRows into the three plane stores. D1-backed
/// implementations call the three D1 bindings. The memory implementation
/// below is for tests and local runs.
pub trait Sink: Send + Sync {
    fn put_intent(&self, row: &PlaneRow) -> Result<(), SinkError>;
    fn put_pattern(&self, row: &PlaneRow) -> Result<(), SinkError>;
    fn put_weave(&self, row: &PlaneRow) -> Result<(), SinkError>;

    /// Write one row to all three planes.
    fn write(&self, row: &PlaneRow) -> Result<(), SinkError> {
        self.put_intent(row)?;
        self.put_pattern(row)?;
        self.put_weave(row)?;
        Ok(())
    }

    /// Write many rows, stopping on first error.
    fn write_all(&self, rows: &[PlaneRow]) -> Result<usize, SinkError> {
        let mut n = 0;
        for row in rows {
            self.write(row)?;
            n += 1;
        }
        Ok(n)
    }
}

/// A row of v_ways_to_intent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WayToIntent {
    pub intent: String,
    pub pattern: String,
    pub weave: String,
    pub occurrences: u64,
    pub first_seen: i64,
    pub last_seen: i64,
}

/// A row of v_intent_frontier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntentFrontier {
    pub intent: String,
    pub patterns: u64,
    pub weaves: u64,
    pub languages: u64,
    pub total_occurrences: u64,
    pub has_singletons: bool,
}

/// A row of v_signature_patterns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignaturePattern {
    pub pattern: String,
    pub source_lang: String,
    pub weaves: Vec<String>,
}

/// In-memory sink. Three maps mirror the three plane schemas.
#[derive(Default, Clone)]
pub struct MemorySink {
    inner: Arc<Mutex<MemorySinkInner>>,
}

/// What's tracked per distinct (weave, pattern, intent) fact. The DIP
/// primary key dedupes on that triple, same as the live table (INSERT
/// OR IGNORE) — so a repeat write of the identical fact does not create
/// a second row. But it is still a second *observation*, and
/// occurrences/langs exist to keep that distinguishable from a fact
/// seen exactly once, which `v_ways_to_intent`'s COUNT(*) and
/// `v_signature_patterns`'s per-language grouping both depend on.
#[derive(Clone)]
struct WeaveMeta {
    // Kept for parity with PlaneRow / future op-filtered queries; no query
    // reads it yet.
    #[allow(dead_code)]
    op: Op,
    source: String,
    langs: std::collections::BTreeSet<String>,
    occurrences: u64,
    first_seen: i64,
    last_seen: i64,
}

#[derive(Default)]
struct MemorySinkInner {
    /// value → binary
    intents: HashMap<String, String>,
    /// pattern → intent
    patterns: HashMap<String, String>,
    /// (weave, pattern, intent) → everything observed about that fact
    weaves: HashMap<(String, String, String), WeaveMeta>,
}

impl MemorySink {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn intent_count(&self) -> usize {
        self.inner.lock().unwrap().intents.len()
    }

    pub fn pattern_count(&self) -> usize {
        self.inner.lock().unwrap().patterns.len()
    }

    pub fn weave_count(&self) -> usize {
        self.inner.lock().unwrap().weaves.len()
    }

    /// All (weave, pattern) ways reaching this intent, with occurrence
    /// counts. This is the `?intent 2` query, mirroring v_ways_to_intent.
    pub fn ways_to(&self, intent: &str) -> Vec<WayToIntent> {
        let inner = self.inner.lock().unwrap();
        inner
            .weaves
            .iter()
            .filter(|((_, _, i), _)| i == intent)
            .map(|((weave, pattern, _), meta)| WayToIntent {
                intent: intent.to_string(),
                pattern: pattern.clone(),
                weave: weave.clone(),
                occurrences: meta.occurrences,
                first_seen: meta.first_seen,
                last_seen: meta.last_seen,
            })
            .collect()
    }

    /// Mirrors v_intent_frontier.
    pub fn frontier(&self, intent: &str) -> IntentFrontier {
        let inner = self.inner.lock().unwrap();
        let mut patterns = std::collections::BTreeSet::new();
        let mut weaves = std::collections::BTreeSet::new();
        let mut languages = std::collections::BTreeSet::new();
        let mut total_occurrences = 0u64;
        let mut has_singletons = false;
        for ((weave, pattern, i), meta) in &inner.weaves {
            if i != intent {
                continue;
            }
            patterns.insert(pattern.clone());
            weaves.insert(weave.clone());
            languages.extend(meta.langs.iter().cloned());
            total_occurrences += meta.occurrences;
            if meta.occurrences == 1 {
                has_singletons = true;
            }
        }
        IntentFrontier {
            intent: intent.to_string(),
            patterns: patterns.len() as u64,
            weaves: weaves.len() as u64,
            languages: languages.len() as u64,
            total_occurrences,
            has_singletons,
        }
    }

    /// Mirrors v_signature_patterns: patterns whose facts all come from
    /// exactly one source_lang, across every weave/intent that pattern
    /// reaches.
    pub fn signature_patterns(&self) -> Vec<SignaturePattern> {
        let inner = self.inner.lock().unwrap();
        let mut by_pattern: BTreeMap<String, (std::collections::BTreeSet<String>, Vec<String>)> =
            BTreeMap::new();
        for ((weave, pattern, _), meta) in &inner.weaves {
            let entry = by_pattern.entry(pattern.clone()).or_default();
            entry.0.extend(meta.langs.iter().cloned());
            entry.1.push(weave.clone());
        }
        let mut out = Vec::new();
        for (pattern, (langs, mut weaves)) in by_pattern {
            if langs.len() == 1 {
                weaves.sort();
                weaves.dedup();
                out.push(SignaturePattern {
                    pattern,
                    source_lang: langs.into_iter().next().unwrap(),
                    weaves,
                });
            }
        }
        out
    }

    /// All intents a given weave produces. This is `?weave +,=`.
    pub fn intents_of_weave(&self, weave: &str) -> Vec<String> {
        let inner = self.inner.lock().unwrap();
        let mut out: Vec<String> = inner
            .weaves
            .keys()
            .filter(|(w, _, _)| w == weave)
            .map(|(_, _, i)| i.clone())
            .collect();
        out.sort();
        out.dedup();
        out
    }

    /// The intent a given pattern reaches.
    pub fn intent_of_pattern(&self, pattern: &str) -> Option<String> {
        self.inner.lock().unwrap().patterns.get(pattern).cloned()
    }

    /// The binary form of a given intent value.
    pub fn binary_of(&self, intent: &str) -> Option<String> {
        self.inner.lock().unwrap().intents.get(intent).cloned()
    }

    pub fn all_witnesses(&self) -> Vec<String> {
        self.inner
            .lock()
            .unwrap()
            .weaves
            .values()
            .map(|meta| meta.source.clone())
            .collect()
    }
}

impl Sink for MemorySink {
    fn put_intent(&self, row: &PlaneRow) -> Result<(), SinkError> {
        if row.intent.is_empty() {
            return Err(SinkError::Invalid("empty intent".into()));
        }
        let mut inner = self.inner.lock().unwrap();
        inner
            .intents
            .entry(row.intent.clone())
            .or_insert_with(|| row.binary.clone());
        Ok(())
    }

    fn put_pattern(&self, row: &PlaneRow) -> Result<(), SinkError> {
        if row.pattern.is_empty() {
            return Err(SinkError::Invalid("empty pattern".into()));
        }
        let mut inner = self.inner.lock().unwrap();
        inner
            .patterns
            .entry(row.pattern.clone())
            .or_insert_with(|| row.intent.clone());
        Ok(())
    }

    fn put_weave(&self, row: &PlaneRow) -> Result<(), SinkError> {
        if row.weave.is_empty() {
            return Err(SinkError::Invalid("empty weave".into()));
        }
        let mut inner = self.inner.lock().unwrap();
        let key = (row.weave.clone(), row.pattern.clone(), row.intent.clone());
        inner
            .weaves
            .entry(key)
            .and_modify(|meta| {
                meta.occurrences += 1;
                meta.langs.insert(row.source_lang.clone());
                meta.last_seen = row.observed_at_ms;
            })
            .or_insert_with(|| {
                let mut langs = std::collections::BTreeSet::new();
                langs.insert(row.source_lang.clone());
                WeaveMeta {
                    op: row.op,
                    source: row.source.clone(),
                    langs,
                    occurrences: 1,
                    first_seen: row.observed_at_ms,
                    last_seen: row.observed_at_ms,
                }
            });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cura_protocol::Triple;

    fn make(op: Op, input: &str, logic: &str, output: &str) -> PlaneRow {
        make_lang(op, input, logic, output, "")
    }

    fn make_lang(op: Op, input: &str, logic: &str, output: &str, lang: &str) -> PlaneRow {
        let s = Sheathed {
            op,
            triple: Triple::new(input, logic, output),
            witness: format!("{input} {logic} {output}").into_bytes(),
            offset: 0,
        };
        PlaneRow::from_sheathed_with_lang(&s, lang)
    }

    #[test]
    fn row_shape_from_2_eq_1_plus_1() {
        let r = make(Op::Change, "1 1", "+", "2");
        assert_eq!(r.intent, "2");
        assert_eq!(
            r.binary,
            "00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000010"
        );
        assert_eq!(r.binary_kind, "i64");
        assert_eq!(r.binary_len, 8);
        assert_eq!(r.pattern, "1_1_2");
        assert_eq!(r.weave, "+,=");
        assert_eq!(r.op, Op::Change);
    }

    #[test]
    fn row_shape_from_mov() {
        let r = make(Op::Read, "[rbp-8]", "mov", "rax");
        assert_eq!(r.intent, "rax");
        // "rax" is a bare identifier, not numeric — a symbol, not bytes().
        assert_eq!(r.binary, "01110010 01100001 01111000");
        assert_eq!(r.binary_kind, "sym");
        assert_eq!(r.binary_len, 3);
        assert_eq!(r.pattern, "[rbp-8]_rax");
        assert_eq!(r.weave, "mov,=");
    }

    #[test]
    fn source_lang_flows_through() {
        let r = make_lang(Op::Change, "1 1", "+", "2", "rust");
        assert_eq!(r.source_lang, "rust");
    }

    #[test]
    fn three_ways_to_two() {
        let sink = MemorySink::new();
        sink.write(&make(Op::Change, "1 1", "+", "2")).unwrap();
        sink.write(&make(Op::Change, "3 1", "-", "2")).unwrap();
        sink.write(&make(Op::Change, "2 1", "*", "2")).unwrap();

        assert_eq!(sink.intent_count(), 1);
        assert_eq!(sink.pattern_count(), 3);
        assert_eq!(sink.weave_count(), 3);
    }

    #[test]
    fn ways_to_returns_occurrences() {
        let sink = MemorySink::new();
        sink.write(&make(Op::Change, "1 1", "+", "2")).unwrap();
        sink.write(&make(Op::Change, "1 1", "+", "2")).unwrap();
        sink.write(&make(Op::Change, "3 1", "-", "2")).unwrap();

        let ways = sink.ways_to("2");
        assert_eq!(ways.len(), 2);

        let plus = ways.iter().find(|w| w.weave == "+,=").unwrap();
        assert_eq!(plus.occurrences, 2);

        let minus = ways.iter().find(|w| w.weave == "-,=").unwrap();
        assert_eq!(minus.occurrences, 1);
    }

    #[test]
    fn frontier_reports_variety() {
        let sink = MemorySink::new();
        sink.write(&make_lang(Op::Change, "1 1", "+", "2", "rust"))
            .unwrap();
        sink.write(&make_lang(Op::Change, "3 1", "-", "2", "python"))
            .unwrap();
        sink.write(&make_lang(Op::Change, "2 1", "*", "2", "rust"))
            .unwrap();

        let f = sink.frontier("2");
        assert_eq!(f.patterns, 3);
        assert_eq!(f.weaves, 3);
        assert_eq!(f.languages, 2);
        assert!(f.has_singletons);
    }

    #[test]
    fn signature_patterns_are_singletons() {
        let sink = MemorySink::new();
        sink.write(&make_lang(Op::Change, "1 1", "+", "2", "rust"))
            .unwrap();
        sink.write(&make_lang(Op::Change, "1 1", "+", "2", "python"))
            .unwrap(); // shared
        sink.write(&make_lang(Op::Change, "10 5", ">>", "2", "zig"))
            .unwrap(); // unique

        let sigs = sink.signature_patterns();
        let patterns: Vec<&str> = sigs.iter().map(|s| s.pattern.as_str()).collect();
        assert!(patterns.contains(&"10_5_2"));
        assert!(!patterns.contains(&"1_1_2"));
    }

    #[test]
    fn duplicate_write_is_deduped() {
        let sink = MemorySink::new();
        let r = make(Op::Change, "1 1", "+", "2");
        sink.write(&r).unwrap();
        sink.write(&r).unwrap();
        sink.write(&r).unwrap();

        assert_eq!(sink.intent_count(), 1);
        assert_eq!(sink.pattern_count(), 1);
        assert_eq!(sink.weave_count(), 1);
    }

    #[test]
    fn query_by_weave() {
        let sink = MemorySink::new();
        sink.write(&make(Op::Change, "1 1", "+", "2")).unwrap();
        sink.write(&make(Op::Change, "3 1", "-", "2")).unwrap();
        sink.write(&make(Op::Change, "5 2", "+", "7")).unwrap();

        let mut intents = sink.intents_of_weave("+,=");
        intents.sort();
        assert_eq!(intents, vec!["2".to_string(), "7".to_string()]);

        let minus = sink.intents_of_weave("-,=");
        assert_eq!(minus, vec!["2".to_string()]);
    }

    #[test]
    fn query_by_pattern() {
        let sink = MemorySink::new();
        sink.write(&make(Op::Change, "1 1", "+", "2")).unwrap();
        assert_eq!(sink.intent_of_pattern("1_1_2"), Some("2".to_string()));
    }

    #[test]
    fn binary_lookup() {
        let sink = MemorySink::new();
        sink.write(&make(Op::Change, "1 1", "+", "2")).unwrap();
        assert_eq!(
            sink.binary_of("2"),
            Some(
                "00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000010"
                    .to_string()
            )
        );
    }

    #[test]
    fn write_all_counts() {
        let sink = MemorySink::new();
        let rows = vec![
            make(Op::Change, "1 1", "+", "2"),
            make(Op::Change, "3 1", "-", "2"),
            make(Op::Change, "5 2", "+", "7"),
        ];
        let n = sink.write_all(&rows).unwrap();
        assert_eq!(n, 3);
    }

    #[test]
    fn empty_intent_rejected() {
        let sink = MemorySink::new();
        let r = make(Op::Write, "", "print", "");
        assert!(matches!(sink.put_intent(&r), Err(SinkError::Invalid(_))));
    }
}
