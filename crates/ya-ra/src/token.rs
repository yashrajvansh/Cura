#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    Int(i64),
    Pattern(Vec<i64>),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Pow,
    Equals,
    LParen,
    RParen,
    Comma,
    Question,
    Newline,
    Eof,
}

impl Token {
    pub fn display(&self) -> String {
        match self {
            Token::Int(n) => n.to_string(),
            Token::Pattern(v) => v
                .iter()
                .map(|x| x.to_string())
                .collect::<Vec<_>>()
                .join("_"),
            Token::Ident(s) => s.clone(),
            Token::Plus => "+".into(),
            Token::Minus => "-".into(),
            Token::Star => "*".into(),
            Token::Slash => "/".into(),
            Token::Percent => "%".into(),
            Token::Pow => "**".into(),
            Token::Equals => "=".into(),
            Token::LParen => "(".into(),
            Token::RParen => ")".into(),
            Token::Comma => ",".into(),
            Token::Question => "?".into(),
            Token::Newline => "\\n".into(),
            Token::Eof => "<eof>".into(),
        }
    }
}
