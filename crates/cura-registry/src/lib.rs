//! Cura — the registry. Stores, indexes, and serves YA|RA.

pub mod binary;
pub mod plane;

pub use binary::{encode as encode_intent, Encoded, Kind};
pub use plane::{
    binary_of_str, byte_len_of_str, kind_of_str, IntentFrontier, MemorySink, PlaneRow,
    SignaturePattern, Sink, SinkError, WayToIntent,
};
