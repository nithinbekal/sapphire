use crate::token::{Token, TokenKind};

pub struct Lexer {
    source: Vec<char>,
    start: usize,
    current: usize,
    line: usize,
}

impl Lexer {
    pub fn new(source: &str) -> Self {
        Self {
            source: source.chars().collect(),
            start: 0,
            current: 0,
            line: 1,
        }
    }

    pub fn scan_tokens(mut self) -> Vec<Token> {
        let mut tokens = Vec::new();

        while !self.is_at_end() {
            self.start = self.current;
            let c = self.advance();

            let kind = match c {
                '(' => TokenKind::LeftParen,
                ')' => TokenKind::RightParen,
                '+' => TokenKind::Plus,
                '-' => TokenKind::Minus,
                '*' => TokenKind::Star,
                '/' => TokenKind::Slash,
                '=' => TokenKind::Eq,
                ';' => TokenKind::Semicolon,
                c if c.is_ascii_digit() => self.number(c),
                c if c.is_ascii_alphabetic() || c == '_' => self.identifier(c),
                _ => continue,
            };

            tokens.push(Token { kind, line: self.line });
        }

        tokens.push(Token { kind: TokenKind::Eof, line: self.line });

        tokens
    }

    fn advance(&mut self) -> char {
        let c = self.source[self.current];
        self.current += 1;
        c
    }

    fn number(&mut self, first: char) -> TokenKind {
        let mut s = String::from(first);
        while !self.is_at_end() && self.source[self.current].is_ascii_digit() {
            s.push(self.advance());
        }
        TokenKind::Number(s.parse().unwrap())
    }

    fn identifier(&mut self, first: char) -> TokenKind {
        let mut s = String::from(first);
        while !self.is_at_end() {
            let c = self.source[self.current];
            if c.is_ascii_alphanumeric() || c == '_' {
                s.push(self.advance());
            } else {
                break;
            }
        }
        match s.as_str() {
            "print" => TokenKind::Print,
            _ => TokenKind::Identifier(s),
        }
    }

    fn is_at_end(&self) -> bool {
        self.current >= self.source.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan(source: &str) -> Vec<TokenKind> {
        Lexer::new(source)
            .scan_tokens()
            .into_iter()
            .map(|t| t.kind)
            .collect()
    }

    #[test]
    fn test_single_operators() {
        assert_eq!(scan("+"), vec![TokenKind::Plus, TokenKind::Eof]);
        assert_eq!(scan("-"), vec![TokenKind::Minus, TokenKind::Eof]);
        assert_eq!(scan("*"), vec![TokenKind::Star, TokenKind::Eof]);
        assert_eq!(scan("/"), vec![TokenKind::Slash, TokenKind::Eof]);
    }

    #[test]
    fn test_sequence() {
        assert_eq!(
            scan("+-*/"),
            vec![
                TokenKind::Plus,
                TokenKind::Minus,
                TokenKind::Star,
                TokenKind::Slash,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn test_ignores_unknown_chars() {
        assert_eq!(scan("  "), vec![TokenKind::Eof]);
        assert_eq!(scan("@"), vec![TokenKind::Eof]);
    }

    #[test]
    fn test_empty() {
        assert_eq!(scan(""), vec![TokenKind::Eof]);
    }

    #[test]
    fn test_integer() {
        assert_eq!(scan("42"), vec![TokenKind::Number(42), TokenKind::Eof]);
        assert_eq!(scan("0"), vec![TokenKind::Number(0), TokenKind::Eof]);
    }

    #[test]
    fn test_integer_in_expression() {
        assert_eq!(
            scan("1+2"),
            vec![
                TokenKind::Number(1),
                TokenKind::Plus,
                TokenKind::Number(2),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn test_identifier() {
        assert_eq!(
            scan("foo"),
            vec![TokenKind::Identifier("foo".into()), TokenKind::Eof]
        );
        assert_eq!(
            scan("my_var"),
            vec![TokenKind::Identifier("my_var".into()), TokenKind::Eof]
        );
    }
}
