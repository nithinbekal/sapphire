use crate::ast::Expr;
use crate::token::{Token, TokenKind};

pub struct Parser {
    tokens: Vec<Token>,
    current: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, current: 0 }
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.current]
    }

    fn is_at_end(&self) -> bool {
        self.peek().kind == TokenKind::Eof
    }

    fn check(&self, kind: &TokenKind) -> bool {
        !self.is_at_end() && &self.peek().kind == kind
    }

    fn advance(&mut self) -> &Token {
        if !self.is_at_end() {
            self.current += 1;
        }
        &self.tokens[self.current - 1]
    }

    pub fn parse(&mut self) -> Expr {
        self.term()
    }

    // term: factor (('+' | '-') factor)*
    fn term(&mut self) -> Expr {
        let mut left = self.factor();

        while self.check(&TokenKind::Plus) || self.check(&TokenKind::Minus) {
            let op = self.advance().clone();
            let right = self.factor();
            left = Expr::Binary {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }

        left
    }

    // factor: primary (('*' | '/') primary)*
    fn factor(&mut self) -> Expr {
        let mut left = self.primary();

        while self.check(&TokenKind::Star) || self.check(&TokenKind::Slash) {
            let op = self.advance().clone();
            let right = self.primary();
            left = Expr::Binary {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }

        left
    }

    // primary: NUMBER | '(' term ')'
    fn primary(&mut self) -> Expr {
        if let TokenKind::Number(n) = self.peek().kind {
            self.advance();
            return Expr::Literal(n);
        }

        if self.check(&TokenKind::LeftParen) {
            self.advance();
            let expr = self.term();
            // consume ')'
            self.advance();
            return Expr::Grouping(Box::new(expr));
        }

        panic!("unexpected token: {:?}", self.peek().kind);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Expr;
    use crate::lexer::Lexer;

    fn parse(source: &str) -> Expr {
        let tokens = Lexer::new(source).scan_tokens();
        Parser::new(tokens).parse()
    }

    #[test]
    fn test_literal() {
        assert!(matches!(parse("42"), Expr::Literal(42)));
    }

    #[test]
    fn test_addition() {
        assert!(matches!(
            parse("1+2"),
            Expr::Binary { .. }
        ));
    }

    #[test]
    fn test_precedence() {
        // 1+2*3 should parse as 1+(2*3), so the top node is Binary(+)
        // whose right child is Binary(*)
        let expr = parse("1+2*3");
        if let Expr::Binary { op, right, .. } = expr {
            assert_eq!(op.kind, TokenKind::Plus);
            assert!(matches!(*right, Expr::Binary { .. }));
        } else {
            panic!("expected Binary");
        }
    }

    #[test]
    fn test_grouping() {
        // (1+2)*3 — top node should be Binary(*) whose left is Grouping
        let expr = parse("(1+2)*3");
        if let Expr::Binary { op, left, .. } = expr {
            assert_eq!(op.kind, TokenKind::Star);
            assert!(matches!(*left, Expr::Grouping(_)));
        } else {
            panic!("expected Binary");
        }
    }
}
