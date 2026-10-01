use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Lex(String),
    Parse(String),
    Eval(String),
    DivisionByZero,
    Overflow,
    InvalidPower,
    UnknownQuery(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Lex(s) => write!(f, "lex: {s}"),
            Error::Parse(s) => write!(f, "parse: {s}"),
            Error::Eval(s) => write!(f, "eval: {s}"),
            Error::DivisionByZero => write!(f, "division by zero"),
            Error::Overflow => write!(f, "integer overflow"),
            Error::InvalidPower => write!(f, "negative exponent"),
            Error::UnknownQuery(s) => write!(f, "unknown query: {s}"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;
