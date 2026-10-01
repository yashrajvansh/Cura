use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Term(pub String);

impl Term {
    pub fn new(s: impl Into<String>) -> Self {
        Term(s.into())
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

/// The atomic unit. The mapping is fixed:
///
///   input  → intent
///   logic  → weave
///   output → pattern
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Triple {
    pub input: Term,
    pub logic: Term,
    pub output: Term,
}

impl Triple {
    pub fn new(i: impl Into<String>, l: impl Into<String>, o: impl Into<String>) -> Self {
        Triple {
            input: Term::new(i),
            logic: Term::new(l),
            output: Term::new(o),
        }
    }

    pub fn is_well_formed(&self) -> bool {
        !(self.input.is_empty() && self.logic.is_empty() && self.output.is_empty())
    }

    /// BLAKE3 over length-prefixed terms. Same value across languages.
    pub fn weave_id(&self) -> WeaveId {
        let mut h = blake3::Hasher::new();
        h.update(b"YA|RA|v1|");
        write_len(&mut h, self.input.as_bytes());
        write_len(&mut h, self.logic.as_bytes());
        write_len(&mut h, self.output.as_bytes());
        WeaveId(*h.finalize().as_bytes())
    }
}

fn write_len(h: &mut blake3::Hasher, b: &[u8]) {
    h.update(&(b.len() as u64).to_be_bytes());
    h.update(b);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WeaveId(pub [u8; 32]);

impl WeaveId {
    pub fn to_hex(&self) -> String {
        const H: &[u8; 16] = b"0123456789abcdef";
        let mut s = String::with_capacity(64);
        for b in &self.0 {
            s.push(H[(b >> 4) as usize] as char);
            s.push(H[(b & 0x0f) as usize] as char);
        }
        s
    }

    pub fn short(&self) -> String {
        let h = self.to_hex();
        format!("{}…{}", &h[..12], &h[h.len() - 8..])
    }

    pub fn shard(&self) -> (String, String) {
        let h = self.to_hex();
        (h[0..2].to_string(), h[2..4].to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable() {
        let t = Triple::new("a", "b", "c");
        assert_eq!(t.weave_id(), t.weave_id());
    }

    #[test]
    fn distinct() {
        assert_ne!(
            Triple::new("a", "b", "c").weave_id(),
            Triple::new("a", "b", "d").weave_id()
        );
    }

    #[test]
    fn no_length_collision() {
        assert_ne!(
            Triple::new("ab", "c", "").weave_id(),
            Triple::new("a", "bc", "").weave_id()
        );
    }

    #[test]
    fn hex_is_64() {
        assert_eq!(Triple::new("x", "y", "z").weave_id().to_hex().len(), 64);
    }
}
