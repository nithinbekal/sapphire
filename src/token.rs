#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    LeftParen,
    RightParen,

    Plus,
    Minus,
    Star,
    Slash,
    Eq,
    Semicolon,

    Number(i64),
    Identifier(String),
    Print,

    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub lexeme: String,
    pub line: usize,
}
