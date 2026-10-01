use crate::error::{Error, Result};
use crate::token::Token;

pub fn lex(input: &str) -> Result<Vec<Token>> {
    let mut tokens = Vec::new();
    let bytes = input.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        let c = bytes[i] as char;

        if c == '\n' {
            tokens.push(Token::Newline);
            i += 1;
            continue;
        }
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c == '#' {
            while i < bytes.len() && bytes[i] as char != '\n' {
                i += 1;
            }
            continue;
        }

        // Multi-char operators
        if i + 1 < bytes.len() && &bytes[i..i + 2] == b"**" {
            tokens.push(Token::Pow);
            i += 2;
            continue;
        }

        match c {
            '+' => {
                tokens.push(Token::Plus);
                i += 1;
            }
            '-' => {
                tokens.push(Token::Minus);
                i += 1;
            }
            '*' => {
                tokens.push(Token::Star);
                i += 1;
            }
            '/' => {
                tokens.push(Token::Slash);
                i += 1;
            }
            '%' => {
                tokens.push(Token::Percent);
                i += 1;
            }
            '=' => {
                tokens.push(Token::Equals);
                i += 1;
            }
            '(' => {
                tokens.push(Token::LParen);
                i += 1;
            }
            ')' => {
                tokens.push(Token::RParen);
                i += 1;
            }
            ',' => {
                tokens.push(Token::Comma);
                i += 1;
            }
            '?' => {
                tokens.push(Token::Question);
                i += 1;
            }
            '0'..='9' => {
                let (tok, next) = read_number(bytes, i)?;
                tokens.push(tok);
                i = next;
            }
            c if c.is_ascii_alphabetic() || c == '_' => {
                let start = i;
                while i < bytes.len() {
                    let ch = bytes[i] as char;
                    if ch.is_ascii_alphanumeric() || ch == '_' {
                        i += 1;
                    } else {
                        break;
                    }
                }
                tokens.push(Token::Ident(input[start..i].to_string()));
            }
            other => {
                return Err(Error::Lex(format!("unexpected character: {other:?}")));
            }
        }
    }

    tokens.push(Token::Eof);
    Ok(tokens)
}

fn read_number(bytes: &[u8], start: usize) -> Result<(Token, usize)> {
    let mut i = start;
    let mut digits = String::new();
    let mut values: Vec<i64> = Vec::new();
    let mut current = String::new();
    let mut has_underscore = false;

    while i < bytes.len() {
        let c = bytes[i] as char;
        if c.is_ascii_digit() {
            current.push(c);
            digits.push(c);
            i += 1;
        } else if c == '_' {
            has_underscore = true;
            let n: i64 = current
                .parse()
                .map_err(|_| Error::Lex(format!("bad number segment: {current:?}")))?;
            values.push(n);
            current.clear();
            i += 1;
        } else {
            break;
        }
    }

    if has_underscore {
        if !current.is_empty() {
            let n: i64 = current
                .parse()
                .map_err(|_| Error::Lex(format!("bad number segment: {current:?}")))?;
            values.push(n);
        } else {
            return Err(Error::Lex("pattern ends with underscore".into()));
        }
        Ok((Token::Pattern(values), i))
    } else {
        let n: i64 = digits
            .parse()
            .map_err(|_| Error::Lex(format!("bad number: {digits:?}")))?;
        Ok((Token::Int(n), i))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexes_equation() {
        let toks = lex("2 = 1 + 1").unwrap();
        assert_eq!(
            toks,
            vec![
                Token::Int(2),
                Token::Equals,
                Token::Int(1),
                Token::Plus,
                Token::Int(1),
                Token::Eof,
            ]
        );
    }

    #[test]
    fn lexes_pattern_literal() {
        let toks = lex("1_1_2").unwrap();
        assert_eq!(toks, vec![Token::Pattern(vec![1, 1, 2]), Token::Eof]);
    }

    #[test]
    fn skips_comments() {
        let toks = lex("2 = 1 + 1 # comment\n").unwrap();
        // comment is skipped, newline kept
        assert!(toks.contains(&Token::Newline));
    }

    #[test]
    fn lexes_pow() {
        let toks = lex("2 ** 1").unwrap();
        assert_eq!(toks[1], Token::Pow);
    }
}
