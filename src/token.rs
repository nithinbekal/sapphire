#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Plus,
    Minus,
    Star,
    Slash,

    Number(i64),

    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub lexeme: String,
    pub line: usize,
}
