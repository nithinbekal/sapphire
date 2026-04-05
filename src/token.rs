#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    LeftParen,
    RightParen,

    Plus,
    Minus,
    Star,
    Slash,
    Bang,
    Eq,
    EqEq,
    BangEq,
    Less,
    LessEq,
    Greater,
    GreaterEq,
    Semicolon,

    LeftBrace,
    RightBrace,

    Number(i64),
    Identifier(String),
    True,
    False,
    If,
    Else,
    While,
    Print,

    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
}
