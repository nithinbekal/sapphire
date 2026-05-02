use crate::token::{Token, TokenKind};

pub struct Lexer {
    source: Vec<char>,
    start: usize,
    current: usize,
    line: usize,
    /// 1-based column of the next character to be consumed.
    current_column: usize,
}

impl Lexer {
    pub fn new(source: &str) -> Self {
        Self {
            source: source.chars().collect(),
            start: 0,
            current: 0,
            line: 1,
            current_column: 1,
        }
    }

    /// Recompute `current_column` by scanning back to the last newline.
    /// Called after scanning constructs that use raw `self.current` increments.
    fn recalc_column(&mut self) {
        let mut col = 1usize;
        for &c in &self.source[..self.current] {
            if c == '\n' {
                col = 1;
            } else {
                col += 1;
            }
        }
        self.current_column = col;
    }

    fn ends_statement(kind: &TokenKind) -> bool {
        matches!(
            kind,
            TokenKind::Identifier(_)
                | TokenKind::Number(_)
                | TokenKind::Float(_)
                | TokenKind::End
                | TokenKind::StringLit(_)
                | TokenKind::StringInterp(_)
                | TokenKind::True
                | TokenKind::False
                | TokenKind::Nil
                | TokenKind::SelfKw
                | TokenKind::SuperKw
                | TokenKind::Yield
                | TokenKind::RightParen
                | TokenKind::RightBracket
                | TokenKind::RightBrace
        )
    }

    pub fn scan_tokens(mut self) -> Vec<Token> {
        let mut tokens = Vec::new();
        let mut last_kind: Option<TokenKind> = None;

        while !self.is_at_end() {
            self.start = self.current;
            let tok_col = self.current_column;
            let c = self.advance();

            let kind = match c {
                '\n' => {
                    self.line += 1;
                    self.current_column = 1;
                    if last_kind.as_ref().is_some_and(Self::ends_statement) {
                        TokenKind::Newline
                    } else {
                        continue;
                    }
                }
                '\r' => continue,
                '(' => TokenKind::LeftParen,
                ')' => TokenKind::RightParen,
                '+' => TokenKind::Plus,
                '-' => {
                    if self.match_next('>') {
                        TokenKind::Arrow
                    } else {
                        TokenKind::Minus
                    }
                }
                '*' => TokenKind::Star,
                '/' => TokenKind::Slash,
                '%' => TokenKind::Percent,
                '!' => {
                    if self.match_next('=') {
                        TokenKind::BangEq
                    } else {
                        TokenKind::Bang
                    }
                }
                '=' => {
                    if self.match_next('=') {
                        TokenKind::EqEq
                    } else {
                        TokenKind::Eq
                    }
                }
                '&' => {
                    if self.match_next('&') {
                        TokenKind::AmpAmp
                    } else if self.match_next('.') {
                        TokenKind::AmpDot
                    } else {
                        TokenKind::Amp
                    }
                }
                '|' => {
                    if self.match_next('|') {
                        TokenKind::PipePipe
                    } else {
                        TokenKind::Pipe
                    }
                }
                '^' => TokenKind::Caret,
                '~' => TokenKind::Tilde,
                '<' => {
                    if self.match_next('<') {
                        TokenKind::LessLess
                    } else if self.match_next('=') {
                        TokenKind::LessEq
                    } else {
                        TokenKind::Less
                    }
                }
                '>' => {
                    if self.match_next('>') {
                        TokenKind::GreaterGreater
                    } else if self.match_next('=') {
                        TokenKind::GreaterEq
                    } else {
                        TokenKind::Greater
                    }
                }
                '{' => TokenKind::LeftBrace,
                '}' => TokenKind::RightBrace,
                '[' => TokenKind::LeftBracket,
                ']' => TokenKind::RightBracket,
                '.' => {
                    if self.match_next('.') {
                        TokenKind::DotDot
                    } else {
                        TokenKind::Dot
                    }
                }
                ':' => TokenKind::Colon,
                ';' => TokenKind::Semicolon,
                ',' => TokenKind::Comma,
                '"' => {
                    if self.current + 1 < self.source.len()
                        && self.source[self.current] == '"'
                        && self.source[self.current + 1] == '"'
                    {
                        match self.heredoc() {
                            Some(s) => {
                                self.recalc_column();
                                s
                            }
                            None => continue,
                        }
                    } else {
                        match self.string() {
                            Some(s) => s,
                            None => continue,
                        }
                    }
                }
                '#' => {
                    while !self.is_at_end() && self.source[self.current] != '\n' {
                        self.current += 1;
                    }
                    continue;
                }
                c if c.is_ascii_digit() => self.number(c),
                c if c.is_ascii_alphabetic() || c == '_' => self.identifier(c),
                '?' => TokenKind::Question,
                _ => continue,
            };

            last_kind = Some(kind.clone());
            tokens.push(Token {
                kind,
                line: self.line,
                column: tok_col,
            });
        }

        tokens.push(Token {
            kind: TokenKind::Eof,
            line: self.line,
            column: self.current_column,
        });

        tokens
    }

    fn match_next(&mut self, expected: char) -> bool {
        if self.is_at_end() || self.source[self.current] != expected {
            return false;
        }
        self.current += 1;
        self.current_column += 1;
        true
    }

    fn advance(&mut self) -> char {
        let c = self.source[self.current];
        self.current += 1;
        self.current_column += 1;
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
                self.current_column += 2;
                let mut depth = 1usize;
                while !self.is_at_end() && depth > 0 {
                    let c = self.advance();
                    match c {
                        '{' => {
                            depth += 1;
                            current.push(c);
                        }
                        '}' => {
                            depth -= 1;
                            if depth > 0 {
                                current.push(c);
                            }
                        }
                        _ => current.push(c),
                    }
                }
                parts.push((current.clone(), true));
                current.clear();
            } else {
                let c = self.advance();
                if c == '\\' && !self.is_at_end() {
                    match self.advance() {
                        'n' => current.push('\n'),
                        't' => current.push('\t'),
                        'r' => current.push('\r'),
                        '\\' => current.push('\\'),
                        '"' => current.push('"'),
                        '#' => current.push('#'),
                        c => {
                            current.push('\\');
                            current.push(c);
                        }
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

    fn heredoc(&mut self) -> Option<TokenKind> {
        self.current += 2; // consume the other two '"' to complete """

        // Opening """ must be followed only by optional whitespace then a newline
        while !self.is_at_end() && self.source[self.current] != '\n' {
            let c = self.source[self.current];
            if c != ' ' && c != '\t' && c != '\r' {
                return None;
            }
            self.current += 1;
        }
        if self.is_at_end() {
            return None;
        }
        self.current += 1; // consume the newline after opening """
        self.line += 1;

        // Collect raw lines until we find a line whose only content is optional indent + """
        let mut raw_lines: Vec<String> = Vec::new();
        let closing_indent;

        loop {
            if self.is_at_end() {
                return None; // unterminated heredoc
            }

            let mut line = String::new();
            while !self.is_at_end() && self.source[self.current] != '\n' {
                let c = self.source[self.current];
                self.current += 1;
                if c == '\r' {
                    continue;
                }
                line.push(c);
            }

            // Check if this line is the closing """
            let indent_len = line
                .chars()
                .take_while(|c| *c == ' ' || *c == '\t')
                .map(|c| c.len_utf8())
                .sum::<usize>();
            let rest = &line[indent_len..];
            if rest == "\"\"\"" {
                closing_indent = line[..indent_len].to_string();
                // Leave the newline after closing """ for the main scan loop
                break;
            }

            raw_lines.push(line);
            if self.is_at_end() {
                return None; // no closing """ found
            }
            self.current += 1; // consume the newline
            self.line += 1;
        }

        // Strip closing_indent prefix from each content line
        let stripped: Vec<String> = raw_lines
            .iter()
            .map(|line| {
                if line.starts_with(&closing_indent) {
                    line[closing_indent.len()..].to_string()
                } else {
                    line.clone()
                }
            })
            .collect();

        let raw_content = stripped.join("\n");

        // Process escape sequences and #{} interpolation on the joined content
        let mut parts: Vec<(String, bool)> = Vec::new();
        let mut current = String::new();
        let mut has_interp = false;
        let chars: Vec<char> = raw_content.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            if chars[i] == '#' && i + 1 < chars.len() && chars[i + 1] == '{' {
                has_interp = true;
                parts.push((current.clone(), false));
                current.clear();
                i += 2; // skip #{
                let mut depth = 1usize;
                while i < chars.len() && depth > 0 {
                    match chars[i] {
                        '{' => {
                            depth += 1;
                            current.push(chars[i]);
                        }
                        '}' => {
                            depth -= 1;
                            if depth > 0 {
                                current.push(chars[i]);
                            }
                        }
                        _ => current.push(chars[i]),
                    }
                    i += 1;
                }
                parts.push((current.clone(), true));
                current.clear();
            } else if chars[i] == '\\' && i + 1 < chars.len() {
                i += 1;
                match chars[i] {
                    'n' => current.push('\n'),
                    't' => current.push('\t'),
                    'r' => current.push('\r'),
                    '\\' => current.push('\\'),
                    '"' => current.push('"'),
                    '#' => current.push('#'),
                    c => {
                        current.push('\\');
                        current.push(c);
                    }
                }
                i += 1;
            } else {
                current.push(chars[i]);
                i += 1;
            }
        }

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
                if ch != '_' {
                    hex.push(ch);
                }
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
            if ch != '_' {
                s.push(ch);
            }
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
                if ch != '_' {
                    s.push(ch);
                }
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
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "nil" => TokenKind::Nil,
            "if" => TokenKind::If,
            "elsif" => TokenKind::Elsif,
            "else" => TokenKind::Else,
            "while" => TokenKind::While,
            "def" => TokenKind::Def,
            "defp" => TokenKind::Defp,
            "return" => TokenKind::Return,
            "break" => TokenKind::Break,
            "next" => TokenKind::Next,
            "class" => TokenKind::Class,
            "module" => TokenKind::Module,
            "include" => TokenKind::Include,
            "attr" => TokenKind::Attr,
            "self" => TokenKind::SelfKw,
            "super" => TokenKind::SuperKw,
            "yield" => TokenKind::Yield,
            "print" => TokenKind::Print,
            "raise" => TokenKind::Raise,
            "begin" => TokenKind::Begin,
            "rescue" => TokenKind::Rescue,
            "end" => TokenKind::End,
            "import" => TokenKind::Import,
            "type" => TokenKind::Type,
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
    fn test_comments() {
        assert_eq!(scan("# hello"), vec![TokenKind::Eof]);
        assert_eq!(
            scan("1 # comment\n2"),
            vec![
                TokenKind::Number(1),
                TokenKind::Newline,
                TokenKind::Number(2),
                TokenKind::Eof
            ]
        );
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
        assert_eq!(
            scan("empty?"),
            vec![TokenKind::Identifier("empty?".into()), TokenKind::Eof]
        );
    }

    #[test]
    fn test_float_literal() {
        assert_eq!(scan("3.14"), vec![TokenKind::Float(3.14), TokenKind::Eof]);
        assert_eq!(scan("1.0"), vec![TokenKind::Float(1.0), TokenKind::Eof]);
    }

    #[test]
    fn test_integer_dot_dot_not_float() {
        // `1..10` must lex as Number(1) DotDot Number(10), not as a float
        assert_eq!(
            scan("1..10"),
            vec![
                TokenKind::Number(1),
                TokenKind::DotDot,
                TokenKind::Number(10),
                TokenKind::Eof
            ]
        );
    }

    #[test]
    fn test_string_escape_sequences() {
        assert_eq!(
            scan(r#""\n""#),
            vec![TokenKind::StringLit("\n".into()), TokenKind::Eof]
        );
        assert_eq!(
            scan(r#""\t""#),
            vec![TokenKind::StringLit("\t".into()), TokenKind::Eof]
        );
        assert_eq!(
            scan(r#""\r""#),
            vec![TokenKind::StringLit("\r".into()), TokenKind::Eof]
        );
        assert_eq!(
            scan(r#""\\""#),
            vec![TokenKind::StringLit("\\".into()), TokenKind::Eof]
        );
        assert_eq!(
            scan(r#""\"""#),
            vec![TokenKind::StringLit("\"".into()), TokenKind::Eof]
        );
        assert_eq!(
            scan(r#""\#""#),
            vec![TokenKind::StringLit("#".into()), TokenKind::Eof]
        );
    }

    #[test]
    fn test_string_escape_unknown_passthrough() {
        // Unknown escape sequences pass through both characters
        assert_eq!(
            scan(r#""\z""#),
            vec![TokenKind::StringLit("\\z".into()), TokenKind::Eof]
        );
    }

}
