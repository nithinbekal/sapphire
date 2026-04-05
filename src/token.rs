#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    LeftParen,
    RightParen,

    Plus,
    Minus,
    Star,
    Slash,
    Bang,
    AmpAmp,
    PipePipe,
    Eq,
    EqEq,
    BangEq,
    Less,
    LessEq,
    Greater,
    GreaterEq,
    Semicolon,
    Comma,

    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Dot,
    Colon,

    Number(i64),
    StringLit(String),
    StringInterp(Vec<(String, bool)>), // (content, is_expr)
    Identifier(String),
    True,
    False,
    If,
    Else,
    While,
    Def,
    Return,
    Class,
    Attr,
    SelfKw,
    Print,

    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
}
