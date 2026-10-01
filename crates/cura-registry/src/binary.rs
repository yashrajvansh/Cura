//! The binary of an intent — what the machine held as the value the
//! intent names.
//!
//! Every intent string is parsed as far as its syntax allows and
//! encoded in the machine's native form for that kind. The kind tag
//! travels with the binary so the bytes are readable back without
//! guessing.
//!
//! Kinds and encodings:
//!
//!   null    — 1 byte, 0x00
//!   bool    — 1 byte, 0x00 or 0x01
//!   i64     — 8 bytes, big-endian, two's complement
//!   u64     — 8 bytes, big-endian, unsigned
//!   f64     — 8 bytes, IEEE 754 binary64, big-endian
//!   f32     — 4 bytes, IEEE 754 binary32, big-endian
//!   char    — 4 bytes, Unicode scalar value, big-endian
//!   str     — UTF-8 bytes (unquoted string)
//!   sym     — UTF-8 bytes of an identifier
//!   bytes   — raw bytes decoded from a hex/bin/oct literal
//!   expr    — UTF-8 bytes of an unparsed expression
//!   hash    — 32 bytes, BLAKE3 of values over 256 bytes
//!
//! Output format: space-separated 8-bit groups, one per byte, most
//! significant bit first. Group order matches the byte order of the
//! encoding (big-endian for multi-byte kinds).
//!
//! Empty input has kind null and empty binary — there is nothing the
//! machine read.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Null,
    Bool,
    I64,
    U64,
    F64,
    F32,
    Char,
    Str,
    Sym,
    Bytes,
    Expr,
    Hash,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Null => "null",
            Kind::Bool => "bool",
            Kind::I64 => "i64",
            Kind::U64 => "u64",
            Kind::F64 => "f64",
            Kind::F32 => "f32",
            Kind::Char => "char",
            Kind::Str => "str",
            Kind::Sym => "sym",
            Kind::Bytes => "bytes",
            Kind::Expr => "expr",
            Kind::Hash => "hash",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "null" => Kind::Null,
            "bool" => Kind::Bool,
            "i64" => Kind::I64,
            "u64" => Kind::U64,
            "f64" => Kind::F64,
            "f32" => Kind::F32,
            "char" => Kind::Char,
            "str" => Kind::Str,
            "sym" => Kind::Sym,
            "bytes" => Kind::Bytes,
            "expr" => Kind::Expr,
            "hash" => Kind::Hash,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Encoded {
    pub kind: Kind,
    pub bytes: Vec<u8>,
}

impl Encoded {
    pub fn binary_string(&self) -> String {
        encode_octets(&self.bytes)
    }

    pub fn byte_len(&self) -> usize {
        self.bytes.len()
    }

    pub fn kind_str(&self) -> &'static str {
        self.kind.as_str()
    }
}

const MAX_INLINE_BYTES: usize = 256;

/// Encode an intent string into its machine representation.
///
/// The parser is deliberately conservative: it only claims a kind when
/// the syntax is unambiguous. A leading-zero digit string is not
/// assumed octal unless prefixed `0o`. A string like `"1_000"` with
/// separators is parsed as an integer after separators are stripped,
/// because every language that allows separators treats them as
/// formatting, not as content.
pub fn encode(s: &str) -> Encoded {
    if s.is_empty() {
        return Encoded {
            kind: Kind::Null,
            bytes: vec![0],
        };
    }

    // Boolean. Only exact true/false in the three common cases.
    match s {
        "true" | "True" | "TRUE" => {
            return Encoded {
                kind: Kind::Bool,
                bytes: vec![1],
            }
        }
        "false" | "False" | "FALSE" => {
            return Encoded {
                kind: Kind::Bool,
                bytes: vec![0],
            }
        }
        _ => {}
    }

    // Null variants.
    match s {
        "null" | "None" | "nil" | "NULL" => {
            return Encoded {
                kind: Kind::Null,
                bytes: vec![0],
            }
        }
        _ => {}
    }

    // Hex literal — bytes decoded.
    if let Some(hex_body) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        let clean: String = hex_body
            .chars()
            .filter(|c| *c != '_' && *c != '\'')
            .collect();
        if !clean.is_empty() && clean.len() % 2 == 0 && clean.chars().all(|c| c.is_ascii_hexdigit())
        {
            if let Some(bytes) = decode_hex(&clean) {
                return Encoded {
                    kind: Kind::Bytes,
                    bytes,
                };
            }
        }
    }

    // Binary literal — bytes decoded.
    if let Some(bin_body) = s.strip_prefix("0b").or_else(|| s.strip_prefix("0B")) {
        let clean: String = bin_body
            .chars()
            .filter(|c| *c != '_' && *c != '\'')
            .collect();
        if !clean.is_empty() && clean.chars().all(|c| c == '0' || c == '1') {
            if let Some(bytes) = decode_bin(&clean) {
                return Encoded {
                    kind: Kind::Bytes,
                    bytes,
                };
            }
        }
    }

    // Octal literal — bytes decoded.
    if let Some(oct_body) = s.strip_prefix("0o").or_else(|| s.strip_prefix("0O")) {
        let clean: String = oct_body
            .chars()
            .filter(|c| *c != '_' && *c != '\'')
            .collect();
        if !clean.is_empty() && clean.chars().all(|c| ('0'..='7').contains(&c)) {
            if let Some(bytes) = decode_oct(&clean) {
                return Encoded {
                    kind: Kind::Bytes,
                    bytes,
                };
            }
        }
    }

    // Quoted string — content bytes.
    if let Some(content) = strip_quotes(s) {
        let bytes = content.as_bytes().to_vec();
        return bounded(Kind::Str, bytes);
    }

    // Single character (bare, not in quotes) — 4-byte scalar.
    //
    // Guard is ASCII-specific on purpose: char::is_alphabetic() counts
    // CJK ideographs (and every other non-Latin letter) as alphabetic,
    // so gating on is_identifier_start here would send a lone `汉` down
    // the Sym path instead of Char — the exact "alphabetic means Latin
    // letter" conflation this parser exists to not make.
    let chars: Vec<char> = s.chars().collect();
    if chars.len() == 1 && !chars[0].is_ascii_alphanumeric() && chars[0] != '_' {
        let cp = chars[0] as u32;
        return Encoded {
            kind: Kind::Char,
            bytes: cp.to_be_bytes().to_vec(),
        };
    }

    // Integer, signed or unsigned.
    let (sign, body) = match s.as_bytes()[0] {
        b'-' => (-1i64, &s[1..]),
        b'+' => (1i64, &s[1..]),
        _ => (1i64, s),
    };
    let stripped_body: String = body.chars().filter(|c| *c != '_' && *c != '\'').collect();
    if !stripped_body.is_empty() && stripped_body.chars().all(|c| c.is_ascii_digit()) {
        // Try i64 first; fall back to u64 for large positive values.
        let full = if sign < 0 {
            format!("-{stripped_body}")
        } else {
            stripped_body.clone()
        };
        if let Ok(n) = full.parse::<i64>() {
            return Encoded {
                kind: Kind::I64,
                bytes: n.to_be_bytes().to_vec(),
            };
        }
        if sign > 0 {
            if let Ok(n) = stripped_body.parse::<u64>() {
                return Encoded {
                    kind: Kind::U64,
                    bytes: n.to_be_bytes().to_vec(),
                };
            }
        }
        // Too big for u64; fall through to bytes.
        return Encoded {
            kind: Kind::Bytes,
            bytes: full.as_bytes().to_vec(),
        };
    }

    // Float, f64 first.
    if looks_like_float(s) {
        if let Ok(f) = s.parse::<f64>() {
            return Encoded {
                kind: Kind::F64,
                bytes: f.to_bits().to_be_bytes().to_vec(),
            };
        }
    }

    // Identifier — symbol. The name is not the value, but it is what
    // the machine held as a reference. Routed through the same
    // 256-byte cap as Str/Expr: a run of `x.repeat(300)` is a
    // syntactically valid identifier, and an oversized value is an
    // oversized value regardless of which kind claims it.
    if is_identifier(s) {
        return bounded(Kind::Sym, s.as_bytes().to_vec());
    }

    // Everything else — expression or unreduced text.
    bounded(Kind::Expr, s.as_bytes().to_vec())
}

fn bounded(kind: Kind, bytes: Vec<u8>) -> Encoded {
    if bytes.len() > MAX_INLINE_BYTES {
        let h = blake3::hash(&bytes);
        Encoded {
            kind: Kind::Hash,
            bytes: h.as_bytes().to_vec(),
        }
    } else {
        Encoded { kind, bytes }
    }
}

fn strip_quotes(s: &str) -> Option<&str> {
    let b = s.as_bytes();
    if b.len() < 2 {
        return None;
    }
    let (open, close) = match b[0] {
        b'"' => (b'"', b'"'),
        b'\'' => (b'\'', b'\''),
        b'`' => (b'`', b'`'),
        _ => return None,
    };
    let _ = open;
    if b[b.len() - 1] != close {
        return None;
    }
    std::str::from_utf8(&b[1..b.len() - 1]).ok()
}

fn looks_like_float(s: &str) -> bool {
    if !(s.contains('.') || s.contains('e') || s.contains('E')) {
        return false;
    }
    s.parse::<f64>().is_ok()
}

fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !is_identifier_start(first) {
        return false;
    }
    chars.all(is_identifier_continue)
}

fn is_identifier_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn is_identifier_continue(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn decode_hex(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        let hi = hex_val(bytes[i])?;
        let lo = hex_val(bytes[i + 1])?;
        out.push((hi << 4) | lo);
        i += 2;
    }
    if i == bytes.len() {
        Some(out)
    } else {
        None
    }
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn decode_bin(s: &str) -> Option<Vec<u8>> {
    // Pad on the left to a multiple of 8.
    let padded = if s.len() % 8 == 0 {
        s.to_string()
    } else {
        format!("{}{}", "0".repeat(8 - s.len() % 8), s)
    };
    let mut out = Vec::with_capacity(padded.len() / 8);
    for chunk in padded.as_bytes().chunks(8) {
        let mut byte = 0u8;
        for &b in chunk {
            byte = (byte << 1) | if b == b'1' { 1 } else { 0 };
        }
        out.push(byte);
    }
    Some(out)
}

fn decode_oct(s: &str) -> Option<Vec<u8>> {
    // Group octal digits in threes; pad left to multiple of 3.
    let padded = if s.len() % 3 == 0 {
        s.to_string()
    } else {
        format!("{}{}", "0".repeat(3 - s.len() % 3), s)
    };
    let mut out = Vec::with_capacity(padded.len() / 3);
    for chunk in padded.as_bytes().chunks(3) {
        let mut val = 0u16;
        for &b in chunk {
            let digit = (b - b'0') as u16;
            if digit > 7 {
                return None;
            }
            val = (val << 3) | digit;
        }
        out.push(val as u8);
    }
    Some(out)
}

fn encode_octets(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 9);
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        for bit in (0..8).rev() {
            out.push(if (b >> bit) & 1 == 1 { '1' } else { '0' });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enc(s: &str) -> Encoded {
        encode(s)
    }

    // -- null and empty --

    #[test]
    fn empty_is_null() {
        let e = enc("");
        assert_eq!(e.kind, Kind::Null);
        assert_eq!(e.bytes, vec![0]);
    }

    #[test]
    fn null_variants() {
        assert_eq!(enc("null").kind, Kind::Null);
        assert_eq!(enc("None").kind, Kind::Null);
        assert_eq!(enc("nil").kind, Kind::Null);
        assert_eq!(enc("NULL").kind, Kind::Null);
    }

    // -- bool --

    #[test]
    fn bool_variants() {
        assert_eq!(enc("true").bytes, vec![1]);
        assert_eq!(enc("True").bytes, vec![1]);
        assert_eq!(enc("TRUE").bytes, vec![1]);
        assert_eq!(enc("false").bytes, vec![0]);
        assert_eq!(enc("False").bytes, vec![0]);
        assert_eq!(enc("FALSE").bytes, vec![0]);
    }

    // -- i64 --

    #[test]
    fn positive_i64() {
        let e = enc("2");
        assert_eq!(e.kind, Kind::I64);
        assert_eq!(e.bytes, vec![0, 0, 0, 0, 0, 0, 0, 2]);
    }

    #[test]
    fn negative_i64() {
        let e = enc("-1");
        assert_eq!(e.kind, Kind::I64);
        assert_eq!(e.bytes, vec![0xFF; 8]);
    }

    #[test]
    fn int64_max() {
        let e = enc("9223372036854775807");
        assert_eq!(e.kind, Kind::I64);
        assert_eq!(
            e.bytes,
            vec![0x7F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
        );
    }

    #[test]
    fn numeric_separators_stripped() {
        let e = enc("1_000");
        assert_eq!(e.kind, Kind::I64);
        assert_eq!(e.bytes, vec![0, 0, 0, 0, 0, 0, 0x03, 0xE8]);
    }

    #[test]
    fn numeric_separators_single_quote() {
        let e = enc("1'000");
        assert_eq!(e.kind, Kind::I64);
        assert_eq!(e.bytes, vec![0, 0, 0, 0, 0, 0, 0x03, 0xE8]);
    }

    #[test]
    fn leading_zero_is_decimal() {
        // "010" is not octal (no prefix). Parse as decimal.
        let e = enc("010");
        assert_eq!(e.kind, Kind::I64);
        assert_eq!(e.bytes[7], 10);
    }

    #[test]
    fn unsigned_above_i64_max() {
        let e = enc("18446744073709551615");
        assert_eq!(e.kind, Kind::U64);
        assert_eq!(e.bytes, vec![0xFF; 8]);
    }

    // -- f64 --

    #[test]
    fn f64_half() {
        let e = enc("0.5");
        assert_eq!(e.kind, Kind::F64);
        // IEEE 754 double: 0x3FE0000000000000.
        assert_eq!(e.bytes, vec![0x3F, 0xE0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn f64_negative() {
        let e = enc("-2.5");
        assert_eq!(e.kind, Kind::F64);
        assert_eq!(e.bytes.len(), 8);
    }

    #[test]
    fn f64_scientific() {
        let e = enc("1e10");
        assert_eq!(e.kind, Kind::F64);
    }

    #[test]
    fn f64_scientific_upper() {
        let e = enc("1E10");
        assert_eq!(e.kind, Kind::F64);
    }

    // -- base literals --

    #[test]
    fn hex_two_digits_is_one_byte() {
        let e = enc("0xFF");
        assert_eq!(e.kind, Kind::Bytes);
        assert_eq!(e.bytes, vec![0xFF]);
    }

    #[test]
    fn hex_four_digits_is_two_bytes() {
        let e = enc("0xDEAD");
        assert_eq!(e.kind, Kind::Bytes);
        assert_eq!(e.bytes, vec![0xDE, 0xAD]);
    }

    #[test]
    fn hex_with_underscore() {
        let e = enc("0xDE_AD");
        assert_eq!(e.kind, Kind::Bytes);
        assert_eq!(e.bytes, vec![0xDE, 0xAD]);
    }

    #[test]
    fn bin_literal() {
        let e = enc("0b11111111");
        assert_eq!(e.kind, Kind::Bytes);
        assert_eq!(e.bytes, vec![0xFF]);
    }

    #[test]
    fn bin_padded() {
        let e = enc("0b1010");
        assert_eq!(e.kind, Kind::Bytes);
        // Padded to 8 bits on the left: 00001010.
        assert_eq!(e.bytes, vec![0x0A]);
    }

    #[test]
    fn oct_literal() {
        let e = enc("0o377");
        assert_eq!(e.kind, Kind::Bytes);
        assert_eq!(e.bytes, vec![0xFF]);
    }

    // -- strings and chars --

    #[test]
    fn quoted_double() {
        let e = enc("\"hello\"");
        assert_eq!(e.kind, Kind::Str);
        assert_eq!(e.bytes, b"hello".to_vec());
    }

    #[test]
    fn quoted_single() {
        let e = enc("'hello'");
        assert_eq!(e.kind, Kind::Str);
        assert_eq!(e.bytes, b"hello".to_vec());
    }

    #[test]
    fn quoted_backtick() {
        let e = enc("`hello`");
        assert_eq!(e.kind, Kind::Str);
        assert_eq!(e.bytes, b"hello".to_vec());
    }

    #[test]
    fn empty_string_literal() {
        let e = enc("\"\"");
        assert_eq!(e.kind, Kind::Str);
        assert!(e.bytes.is_empty());
    }

    #[test]
    fn single_char_literal_bare() {
        let e = enc("+");
        assert_eq!(e.kind, Kind::Char);
        // Unicode scalar for '+' is U+002B, 4-byte big-endian.
        assert_eq!(e.bytes, vec![0, 0, 0, 0x2B]);
    }

    #[test]
    fn single_cjk_char_literal() {
        let e = enc("汉");
        assert_eq!(e.kind, Kind::Char);
        // U+6C49.
        assert_eq!(e.bytes, vec![0, 0, 0x6C, 0x49]);
    }

    // -- symbols --

    #[test]
    fn simple_symbol() {
        let e = enc("version");
        assert_eq!(e.kind, Kind::Sym);
        assert_eq!(e.bytes, b"version".to_vec());
    }

    #[test]
    fn symbol_with_digits() {
        let e = enc("x2");
        assert_eq!(e.kind, Kind::Sym);
        assert_eq!(e.bytes, b"x2".to_vec());
    }

    #[test]
    fn symbol_with_underscore() {
        let e = enc("my_var");
        assert_eq!(e.kind, Kind::Sym);
        assert_eq!(e.bytes, b"my_var".to_vec());
    }

    #[test]
    fn cjk_symbol() {
        let e = enc("变量");
        assert_eq!(e.kind, Kind::Sym);
        assert_eq!(e.bytes, "变量".as_bytes().to_vec());
    }

    // -- expressions --

    #[test]
    fn call_expression() {
        let e = enc("fclose(f)");
        assert_eq!(e.kind, Kind::Expr);
        assert_eq!(e.bytes, b"fclose(f)".to_vec());
    }

    #[test]
    fn dotted_expression() {
        let e = enc("a.b.c");
        assert_eq!(e.kind, Kind::Expr);
        assert_eq!(e.bytes, b"a.b.c".to_vec());
    }

    // -- hashing --

    #[test]
    fn long_input_hashes() {
        // A trailing '.' keeps this from parsing as a valid identifier,
        // so it actually exercises the Expr->Hash boundary rather than
        // the (separately capped) Sym path.
        let s = format!("{}.", "x".repeat(299));
        assert_eq!(s.len(), 300);
        let e = enc(&s);
        assert_eq!(e.kind, Kind::Hash);
        assert_eq!(e.bytes.len(), 32);
    }

    #[test]
    fn exactly_256_is_not_hashed() {
        let s = format!("{}.", "x".repeat(255));
        assert_eq!(s.len(), 256);
        let e = enc(&s);
        assert_eq!(e.kind, Kind::Expr);
        assert_eq!(e.bytes.len(), 256);
    }

    #[test]
    fn one_over_is_hashed() {
        let s = format!("{}.", "x".repeat(256));
        assert_eq!(s.len(), 257);
        let e = enc(&s);
        assert_eq!(e.kind, Kind::Hash);
    }

    #[test]
    fn long_identifier_is_also_capped() {
        // A run of `x`s is a syntactically valid identifier, and an
        // oversized identifier is still an oversized value.
        let s = "x".repeat(300);
        let e = enc(&s);
        assert_eq!(e.kind, Kind::Hash);
        assert_eq!(e.bytes.len(), 32);
    }

    // -- output format --

    #[test]
    fn binary_string_format() {
        let e = enc("2");
        let b = e.binary_string();
        assert_eq!(
            b,
            "00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000010"
        );
    }

    #[test]
    fn null_binary_string() {
        let e = enc("");
        assert_eq!(e.binary_string(), "00000000");
    }

    #[test]
    fn sym_binary_string() {
        let e = enc("hi");
        assert_eq!(e.binary_string(), "01101000 01101001");
    }
}
