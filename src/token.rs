#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    LeftParen,
    RightParen,

    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Bang,
    Pipe,
    AmpDot,
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
    Nil,
    If,
    Else,
    While,
    Def,
    Return,
    Break,
    Next,
    Class,
    Attr,
    SelfKw,
    SuperKw,
    Print,

    Newline,
    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
}
