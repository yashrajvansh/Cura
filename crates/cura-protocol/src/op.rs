//! The five operations every language is built from, and what a reader
//! produces when it recognizes one. Readers (curator's sheaths) make
//! these; the registry stores them.

use crate::Triple;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Op {
    Read,
    Change,
    Branch,
    Write,
    Repeat,
}

impl Op {
    pub fn as_str(self) -> &'static str {
        match self {
            Op::Read => "read",
            Op::Change => "change",
            Op::Branch => "branch",
            Op::Write => "write",
            Op::Repeat => "repeat",
        }
    }

    pub fn all() -> [Op; 5] {
        [Op::Read, Op::Change, Op::Branch, Op::Write, Op::Repeat]
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "read" => Some(Op::Read),
            "change" => Some(Op::Change),
            "branch" => Some(Op::Branch),
            "write" => Some(Op::Write),
            "repeat" => Some(Op::Repeat),
            _ => None,
        }
    }
}

/// What the sheath produces. A triple per recognized operation, and
/// the raw bytes that triggered the recognition. The witness is what
/// gets recorded — not the language, not the parser's guess about
/// meaning.
#[derive(Clone, Debug)]
pub struct Sheathed {
    pub op: Op,
    pub triple: Triple,
    pub witness: Vec<u8>,
    pub offset: usize,
}

/// A sheath reads bytes and wraps them into triples. Every sheath
/// implements this trait. Different sheaths might recognize different
/// alphabets, but the shape of what they produce is the same.
pub trait Sheath: Send + Sync {
    fn name(&self) -> &'static str;
    fn wrap(&self, bytes: &[u8]) -> Vec<Sheathed>;
}
