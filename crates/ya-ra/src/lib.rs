//! YA|RA — a language whose evaluation produces a registry.
//!
//! Every statement is an equation. Running an equation records three
//! facts: the intent (the value it produces, with its binary form),
//! the pattern (the leaf literals joined with the answer), and the
//! weave (the set of operators used).
//!
//! Queries ask the registry what patterns and weaves reach a given
//! intent, what intent a pattern reaches, or what intents a weave
//! produces. The language does not compute for you. It catalogs.

pub mod ast;
pub mod error;
pub mod eval;
pub mod lexer;
pub mod parser;
pub mod registry;
pub mod token;

pub use error::Error;
pub use eval::Interpreter;
pub use registry::Registry;
