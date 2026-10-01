use crate::ast::Op;
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Intent {
    pub value: i64,
    pub binary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub intent: Intent,
    pub pattern: Vec<i64>,
    pub weave: BTreeSet<Op>,
}

#[derive(Debug, Default)]
pub struct Registry {
    entries: Vec<Entry>,
}

impl Registry {
    pub fn new() -> Self {
        Registry::default()
    }

    pub fn record(&mut self, intent: Intent, pattern: Vec<i64>, weave: BTreeSet<Op>) {
        // Deduplicate identical (intent, pattern, weave) triples.
        for e in &self.entries {
            if e.intent == intent && e.pattern == pattern && e.weave == weave {
                return;
            }
        }
        self.entries.push(Entry {
            intent,
            pattern,
            weave,
        });
    }

    pub fn by_intent(&self, value: i64) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|e| e.intent.value == value)
            .collect()
    }

    pub fn by_pattern(&self, pattern: &[i64]) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|e| e.pattern == pattern)
            .collect()
    }

    pub fn by_weave(&self, weave: &BTreeSet<Op>) -> Vec<&Entry> {
        self.entries.iter().filter(|e| &e.weave == weave).collect()
    }

    pub fn all(&self) -> &[Entry] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Number of distinct intent values seen.
    pub fn distinct_intents(&self) -> usize {
        let mut s = BTreeSet::new();
        for e in &self.entries {
            s.insert(e.intent.value);
        }
        s.len()
    }

    /// Number of distinct patterns seen.
    pub fn distinct_patterns(&self) -> usize {
        let mut s = BTreeSet::new();
        for e in &self.entries {
            s.insert(e.pattern.clone());
        }
        s.len()
    }
}

pub fn to_binary(n: i64) -> String {
    if n == 0 {
        return "0".to_string();
    }
    let neg = n < 0;
    let mut v = n.unsigned_abs();
    let mut s = String::new();
    while v > 0 {
        s.push(if v & 1 == 1 { '1' } else { '0' });
        v >>= 1;
    }
    let bits: String = s.chars().rev().collect();
    if neg {
        format!("-{bits}")
    } else {
        bits
    }
}

pub fn weave_to_string(weave: &BTreeSet<Op>) -> String {
    let mut parts: Vec<String> = weave.iter().map(|o| o.symbol().to_string()).collect();
    parts.push("=".to_string());
    parts.join(",")
}

pub fn pattern_to_string(pattern: &[i64]) -> String {
    pattern
        .iter()
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join("_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_of_2() {
        assert_eq!(to_binary(2), "10");
    }

    #[test]
    fn binary_of_0() {
        assert_eq!(to_binary(0), "0");
    }

    #[test]
    fn binary_of_negative() {
        assert_eq!(to_binary(-2), "-10");
    }

    #[test]
    fn pattern_display() {
        assert_eq!(pattern_to_string(&[1, 1, 2]), "1_1_2");
    }
}
