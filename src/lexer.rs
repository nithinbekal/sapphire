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

    fn ends_statement(kind: &TokenKind) -> bool {
        matches!(kind,
            TokenKind::Identifier(_) | TokenKind::Number(_) | TokenKind::Float(_) |
            TokenKind::End |
            TokenKind::StringLit(_) | TokenKind::StringInterp(_) |
            TokenKind::True | TokenKind::False | TokenKind::Nil |
            TokenKind::SelfKw | TokenKind::SuperKw | TokenKind::Yield |
            TokenKind::RightParen | TokenKind::RightBracket | TokenKind::RightBrace
        )
    }

    pub fn scan_tokens(mut self) -> Vec<Token> {
        let mut tokens = Vec::new();
        let mut last_kind: Option<TokenKind> = None;

        while !self.is_at_end() {
            self.start = self.current;
            let c = self.advance();

            let kind = match c {
                '\n' => {
                    self.line += 1;
                    if last_kind.as_ref().map_or(false, Self::ends_statement) {
                        TokenKind::Newline
                    } else {
                        continue
                    }
                }
                '\r' => continue,
                '(' => TokenKind::LeftParen,
                ')' => TokenKind::RightParen,
                '+' => TokenKind::Plus,
                '-' => if self.match_next('>') { TokenKind::Arrow } else { TokenKind::Minus },
                '*' => TokenKind::Star,
                '/' => TokenKind::Slash,
                '%' => TokenKind::Percent,
                '!' => if self.match_next('=') { TokenKind::BangEq } else { TokenKind::Bang },
                '=' => if self.match_next('=') { TokenKind::EqEq } else { TokenKind::Eq },
                '&' => {
                    if self.match_next('&') { TokenKind::AmpAmp }
                    else if self.match_next('.') { TokenKind::AmpDot }
                    else { TokenKind::Amp }
                }
                '|' => if self.match_next('|') { TokenKind::PipePipe } else { TokenKind::Pipe },
                '^' => TokenKind::Caret,
                '~' => TokenKind::Tilde,
                '<' => {
                    if self.match_next('<') { TokenKind::LessLess }
                    else if self.match_next('=') { TokenKind::LessEq }
                    else { TokenKind::Less }
                }
                '>' => {
                    if self.match_next('>') { TokenKind::GreaterGreater }
                    else if self.match_next('=') { TokenKind::GreaterEq }
                    else { TokenKind::Greater }
                }
                '{' => TokenKind::LeftBrace,
                '}' => TokenKind::RightBrace,
                '[' => TokenKind::LeftBracket,
                ']' => TokenKind::RightBracket,
                '.' => if self.match_next('.') { TokenKind::DotDot } else { TokenKind::Dot },
                ':' => TokenKind::Colon,
                ';' => TokenKind::Semicolon,
                ',' => TokenKind::Comma,
                '"' => match self.string() {
                    Some(s) => s,
                    None => continue,
                },
                '#' => {
                    while !self.is_at_end() && self.source[self.current] != '\n' {
                        self.current += 1;
                    }
                    continue;
                }
                c if c.is_ascii_digit() => self.number(c),
                c if c.is_ascii_alphabetic() || c == '_' => self.identifier(c),
                _ => continue,
            };

            last_kind = Some(kind.clone());
            tokens.push(Token { kind, line: self.line });
        }

        tokens.push(Token { kind: TokenKind::Eof, line: self.line });

        tokens
    }

    fn match_next(&mut self, expected: char) -> bool {
        if self.is_at_end() || self.source[self.current] != expected {
            return false;
        }
        self.current += 1;
        true
    }

    fn advance(&mut self) -> char {
        let c = self.source[self.current];
        self.current += 1;
        c
    }

    fn string(&mut self) -> Option<TokenKind> {
        let mut parts: Vec<(String, bool)> = Vec::new();
        let mut current = String::new();
        let mut has_interp = false;

        while !self.is_at_end() && self.source[self.current] != '"' {
            if self.source[self.current] == '#'
                && self.current + 1 < self.source.len()
                && self.source[self.current + 1] == '{'
            {
                has_interp = true;
                parts.push((current.clone(), false));
                current.clear();
                self.current += 2; // skip #{
                let mut depth = 1usize;
                while !self.is_at_end() && depth > 0 {
                    let c = self.advance();
                    match c {
                        '{' => { depth += 1; current.push(c); }
                        '}' => { depth -= 1; if depth > 0 { current.push(c); } }
                        _ => current.push(c),
                    }
                }
                parts.push((current.clone(), true));
                current.clear();
            } else {
                let c = self.advance();
                if c == '\\' && !self.is_at_end() {
                    match self.advance() {
                        'n'  => current.push('\n'),
                        't'  => current.push('\t'),
                        'r'  => current.push('\r'),
                        '\\' => current.push('\\'),
                        '"'  => current.push('"'),
                        '#'  => current.push('#'),
                        c    => { current.push('\\'); current.push(c); }
                    }
                } else {
                    current.push(c);
                }
            }
        }
        if self.is_at_end() {
            return None; // unterminated string
        }
        self.advance(); // consume closing '"'

        if !has_interp {
            return Some(TokenKind::StringLit(current));
        }
        if !current.is_empty() {
            parts.push((current, false));
        }
        Some(TokenKind::StringInterp(parts))
    }

    fn number(&mut self, first: char) -> TokenKind {
        // Hex literal: 0xFF, 0xDEAD_BEEF, etc.
        if first == '0'
            && !self.is_at_end()
            && (self.source[self.current] == 'x' || self.source[self.current] == 'X')
        {
            self.advance(); // consume 'x'/'X'
            let mut hex = String::new();
            while !self.is_at_end()
                && (self.source[self.current].is_ascii_hexdigit()
                    || (self.source[self.current] == '_'
                        && self.current + 1 < self.source.len()
                        && self.source[self.current + 1].is_ascii_hexdigit()))
            {
                let ch = self.advance();
                if ch != '_' { hex.push(ch); }
            }
            return TokenKind::Number(i64::from_str_radix(&hex, 16).unwrap());
        }

        let mut s = String::from(first);
        while !self.is_at_end()
            && (self.source[self.current].is_ascii_digit()
                || (self.source[self.current] == '_'
                    && self.current + 1 < self.source.len()
                    && self.source[self.current + 1].is_ascii_digit()))
        {
            let ch = self.advance();
            if ch != '_' { s.push(ch); }
        }
        // Consume `.digits` as the fractional part of a float.
        // Guard: next char is `.` AND the char after that is a digit (not `..`).
        if !self.is_at_end()
            && self.source[self.current] == '.'
            && self.current + 1 < self.source.len()
            && self.source[self.current + 1].is_ascii_digit()
        {
            s.push(self.advance()); // '.'
            while !self.is_at_end()
                && (self.source[self.current].is_ascii_digit()
                    || (self.source[self.current] == '_'
                        && self.current + 1 < self.source.len()
                        && self.source[self.current + 1].is_ascii_digit()))
            {
                let ch = self.advance();
                if ch != '_' { s.push(ch); }
            }
            TokenKind::Float(s.parse().unwrap())
        } else {
            TokenKind::Number(s.parse().unwrap())
        }
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
        if !self.is_at_end() && self.source[self.current] == '?' {
            s.push(self.advance());
        }
        match s.as_str() {
            "true"  => TokenKind::True,
            "false" => TokenKind::False,
            "nil"   => TokenKind::Nil,
            "if"    => TokenKind::If,
            "elsif" => TokenKind::Elsif,
            "else"  => TokenKind::Else,
            "while" => TokenKind::While,
            "def"    => TokenKind::Def,
            "defp"   => TokenKind::Defp,
            "return" => TokenKind::Return,
            "break"  => TokenKind::Break,
            "next"   => TokenKind::Next,
            "class"  => TokenKind::Class,
            "attr"   => TokenKind::Attr,
            "self"   => TokenKind::SelfKw,
            "super"  => TokenKind::SuperKw,
            "yield"  => TokenKind::Yield,
            "print"  => TokenKind::Print,
            "raise"  => TokenKind::Raise,
            "begin"  => TokenKind::Begin,
            "rescue" => TokenKind::Rescue,
            "end"    => TokenKind::End,
            "import" => TokenKind::Import,
            _ => TokenKind::Identifier(s),
        }
    }

    fn is_at_end(&self) -> bool {
        self.current >= self.source.len()
    }
}
