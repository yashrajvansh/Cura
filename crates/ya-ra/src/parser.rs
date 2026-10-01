use crate::ast::{Expr, Op, Stmt};
use crate::error::{Error, Result};
use crate::token::Token;
use std::collections::BTreeSet;

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn advance(&mut self) -> Token {
        let t = self.tokens[self.pos].clone();
        self.pos += 1;
        t
    }

    fn expect(&mut self, expected: &Token) -> Result<()> {
        let got = self.advance();
        if &got != expected {
            return Err(Error::Parse(format!(
                "expected {}, got {}",
                expected.display(),
                got.display()
            )));
        }
        Ok(())
    }

    fn skip_newlines(&mut self) {
        while matches!(self.peek(), Token::Newline) {
            self.pos += 1;
        }
    }

    pub fn parse_program(&mut self) -> Result<Vec<Stmt>> {
        let mut stmts = Vec::new();
        self.skip_newlines();
        while !matches!(self.peek(), Token::Eof) {
            stmts.push(self.parse_statement()?);
            self.skip_newlines();
        }
        Ok(stmts)
    }

    fn parse_statement(&mut self) -> Result<Stmt> {
        if matches!(self.peek(), Token::Question) {
            self.parse_query()
        } else {
            self.parse_equation()
        }
    }

    fn parse_equation(&mut self) -> Result<Stmt> {
        let lhs = self.parse_expr()?;
        self.expect(&Token::Equals)?;
        let rhs = self.parse_expr()?;
        Ok(Stmt::Equation(lhs, rhs))
    }

    fn parse_query(&mut self) -> Result<Stmt> {
        self.expect(&Token::Question)?;
        let keyword = match self.advance() {
            Token::Ident(s) => s,
            other => {
                return Err(Error::Parse(format!(
                    "expected query keyword, got {}",
                    other.display()
                )));
            }
        };
        match keyword.as_str() {
            "intent" => {
                let e = self.parse_expr()?;
                Ok(Stmt::QueryIntent(e))
            }
            "pattern" => match self.advance() {
                Token::Pattern(v) => Ok(Stmt::QueryPattern(v)),
                other => Err(Error::Parse(format!(
                    "expected pattern literal, got {}",
                    other.display()
                ))),
            },
            "weave" => {
                let mut ops = BTreeSet::new();
                loop {
                    match self.advance() {
                        Token::Plus => {
                            ops.insert(Op::Add);
                        }
                        Token::Minus => {
                            ops.insert(Op::Sub);
                        }
                        Token::Star => {
                            ops.insert(Op::Mul);
                        }
                        Token::Slash => {
                            ops.insert(Op::Div);
                        }
                        Token::Percent => {
                            ops.insert(Op::Mod);
                        }
                        Token::Pow => {
                            ops.insert(Op::Pow);
                        }
                        Token::Equals => {
                            /* '=' terminates the weave */
                            break;
                        }
                        other => {
                            return Err(Error::Parse(format!(
                                "expected operator or =, got {}",
                                other.display()
                            )));
                        }
                    }
                    if matches!(self.peek(), Token::Comma) {
                        self.advance();
                        continue;
                    }
                    break;
                }
                Ok(Stmt::QueryWeave(ops))
            }
            other => Err(Error::UnknownQuery(other.to_string())),
        }
    }

    fn parse_expr(&mut self) -> Result<Expr> {
        self.parse_additive()
    }

    fn parse_additive(&mut self) -> Result<Expr> {
        let mut left = self.parse_multiplicative()?;
        loop {
            match self.peek() {
                Token::Plus => {
                    self.advance();
                    let right = self.parse_multiplicative()?;
                    left = Expr::BinOp(Box::new(left), Op::Add, Box::new(right));
                }
                Token::Minus => {
                    self.advance();
                    let right = self.parse_multiplicative()?;
                    left = Expr::BinOp(Box::new(left), Op::Sub, Box::new(right));
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr> {
        let mut left = self.parse_power()?;
        loop {
            match self.peek() {
                Token::Star => {
                    self.advance();
                    let right = self.parse_power()?;
                    left = Expr::BinOp(Box::new(left), Op::Mul, Box::new(right));
                }
                Token::Slash => {
                    self.advance();
                    let right = self.parse_power()?;
                    left = Expr::BinOp(Box::new(left), Op::Div, Box::new(right));
                }
                Token::Percent => {
                    self.advance();
                    let right = self.parse_power()?;
                    left = Expr::BinOp(Box::new(left), Op::Mod, Box::new(right));
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_power(&mut self) -> Result<Expr> {
        let base = self.parse_primary()?;
        if matches!(self.peek(), Token::Pow) {
            self.advance();
            let exp = self.parse_power()?;
            Ok(Expr::BinOp(Box::new(base), Op::Pow, Box::new(exp)))
        } else {
            Ok(base)
        }
    }

    fn parse_primary(&mut self) -> Result<Expr> {
        match self.advance() {
            Token::Int(n) => Ok(Expr::Int(n)),
            Token::LParen => {
                let e = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                Ok(e)
            }
            other => Err(Error::Parse(format!(
                "expected number or ( , got {}",
                other.display()
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;

    fn parse(src: &str) -> Vec<Stmt> {
        let toks = lex(src).unwrap();
        Parser::new(toks).parse_program().unwrap()
    }

    #[test]
    fn parses_equation() {
        let stmts = parse("2 = 1 + 1");
        assert_eq!(stmts.len(), 1);
        assert!(matches!(stmts[0], Stmt::Equation(_, _)));
    }

    #[test]
    fn parses_query_intent() {
        let stmts = parse("?intent 2");
        assert_eq!(stmts.len(), 1);
        assert!(matches!(stmts[0], Stmt::QueryIntent(_)));
    }

    #[test]
    fn parses_query_pattern() {
        let stmts = parse("?pattern 1_1_2");
        assert_eq!(stmts.len(), 1);
        match &stmts[0] {
            Stmt::QueryPattern(v) => assert_eq!(v, &vec![1, 1, 2]),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn precedence() {
        let stmts = parse("2 = 1 + 1 * 1");
        // Should parse as 1 + (1 * 1) = 2
        assert_eq!(stmts.len(), 1);
    }
}
