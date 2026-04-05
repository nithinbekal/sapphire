use crate::ast::{Expr, Stmt};
use crate::error::SapphireError;
use crate::token::{Token, TokenKind};
use crate::value::Value;

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

    pub fn parse(&mut self) -> Result<Vec<Stmt>, SapphireError> {
        let mut stmts = Vec::new();
        while !self.is_at_end() {
            stmts.push(self.statement()?);
            if self.check(&TokenKind::Semicolon) {
                self.advance();
            }
        }
        Ok(stmts)
    }

    fn statement(&mut self) -> Result<Stmt, SapphireError> {
        if self.check(&TokenKind::Return) {
            self.advance();
            return Ok(Stmt::Return(self.equality()?));
        }
        if self.check(&TokenKind::Def) {
            return self.function_def();
        }
        if self.check(&TokenKind::If) {
            return self.if_statement();
        }
        if self.check(&TokenKind::While) {
            return self.while_statement();
        }
        if self.check(&TokenKind::Print) {
            self.advance();
            return Ok(Stmt::Print(self.equality()?));
        }
        Ok(Stmt::Expression(self.equality()?))
    }

    fn if_statement(&mut self) -> Result<Stmt, SapphireError> {
        self.advance(); // consume 'if'
        let condition = self.equality()?;
        let then_branch = self.block()?;
        let else_branch = if self.check(&TokenKind::Else) {
            self.advance(); // consume 'else'
            Some(self.block()?)
        } else {
            None
        };
        Ok(Stmt::If { condition, then_branch, else_branch })
    }

    fn function_def(&mut self) -> Result<Stmt, SapphireError> {
        self.advance(); // consume 'def'
        let name = match self.peek().kind.clone() {
            TokenKind::Identifier(n) => { self.advance(); n }
            _ => return Err(SapphireError::ParseError {
                message: "expected function name".into(),
                line: self.peek().line,
            }),
        };
        if !self.check(&TokenKind::LeftParen) {
            return Err(SapphireError::ParseError {
                message: "expected '(' after function name".into(),
                line: self.peek().line,
            });
        }
        self.advance(); // consume '('
        let mut params = Vec::new();
        if !self.check(&TokenKind::RightParen) {
            loop {
                match self.peek().kind.clone() {
                    TokenKind::Identifier(p) => { self.advance(); params.push(p); }
                    _ => return Err(SapphireError::ParseError {
                        message: "expected parameter name".into(),
                        line: self.peek().line,
                    }),
                }
                if !self.check(&TokenKind::Comma) { break; }
                self.advance();
            }
        }
        if !self.check(&TokenKind::RightParen) {
            return Err(SapphireError::ParseError {
                message: "expected ')' after parameters".into(),
                line: self.peek().line,
            });
        }
        self.advance(); // consume ')'
        let body = self.block()?;
        Ok(Stmt::Function { name, params, body })
    }

    fn while_statement(&mut self) -> Result<Stmt, SapphireError> {
        self.advance(); // consume 'while'
        let condition = self.equality()?;
        let body = self.block()?;
        Ok(Stmt::While { condition, body })
    }

    fn block(&mut self) -> Result<Vec<Stmt>, SapphireError> {
        if !self.check(&TokenKind::LeftBrace) {
            return Err(SapphireError::ParseError {
                message: "expected '{'".into(),
                line: self.peek().line,
            });
        }
        self.advance(); // consume '{'
        let mut stmts = Vec::new();
        while !self.check(&TokenKind::RightBrace) && !self.is_at_end() {
            stmts.push(self.statement()?);
            if self.check(&TokenKind::Semicolon) {
                self.advance();
            }
        }
        if !self.check(&TokenKind::RightBrace) {
            return Err(SapphireError::ParseError {
                message: "expected '}'".into(),
                line: self.peek().line,
            });
        }
        self.advance(); // consume '}'
        Ok(stmts)
    }

    // equality: comparison (('==' | '!=') comparison)*
    fn equality(&mut self) -> Result<Expr, SapphireError> {
        let mut left = self.comparison()?;
        while self.check(&TokenKind::EqEq) || self.check(&TokenKind::BangEq) {
            let op = self.advance().clone();
            let right = self.comparison()?;
            left = Expr::Binary { left: Box::new(left), op, right: Box::new(right) };
        }
        Ok(left)
    }

    // comparison: term (('<' | '<=' | '>' | '>=') term)*
    fn comparison(&mut self) -> Result<Expr, SapphireError> {
        let mut left = self.term()?;
        while self.check(&TokenKind::Less) || self.check(&TokenKind::LessEq)
            || self.check(&TokenKind::Greater) || self.check(&TokenKind::GreaterEq)
        {
            let op = self.advance().clone();
            let right = self.term()?;
            left = Expr::Binary { left: Box::new(left), op, right: Box::new(right) };
        }
        Ok(left)
    }

    // term: factor (('+' | '-') factor)*
    fn term(&mut self) -> Result<Expr, SapphireError> {
        let mut left = self.factor()?;

        while self.check(&TokenKind::Plus) || self.check(&TokenKind::Minus) {
            let op = self.advance().clone();
            let right = self.factor()?;
            left = Expr::Binary {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    // factor: unary (('*' | '/') unary)*
    fn factor(&mut self) -> Result<Expr, SapphireError> {
        let mut left = self.unary()?;

        while self.check(&TokenKind::Star) || self.check(&TokenKind::Slash) {
            let op = self.advance().clone();
            let right = self.unary()?;
            left = Expr::Binary {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    // unary: ('!' | '-') unary | primary
    fn unary(&mut self) -> Result<Expr, SapphireError> {
        if self.check(&TokenKind::Bang) || self.check(&TokenKind::Minus) {
            let op = self.advance().clone();
            let right = self.unary()?;
            return Ok(Expr::Unary { op, right: Box::new(right) });
        }
        self.primary()
    }

    // primary: NUMBER | BOOL | IDENTIFIER ('=' equality)? | '(' equality ')'
    fn primary(&mut self) -> Result<Expr, SapphireError> {
        if let TokenKind::Number(n) = self.peek().kind {
            self.advance();
            return Ok(Expr::Literal(Value::Int(n)));
        }

        if let TokenKind::StringLit(s) = self.peek().kind.clone() {
            self.advance();
            return Ok(Expr::Literal(Value::Str(s)));
        }

        if self.check(&TokenKind::True) {
            self.advance();
            return Ok(Expr::Literal(Value::Bool(true)));
        }

        if self.check(&TokenKind::False) {
            self.advance();
            return Ok(Expr::Literal(Value::Bool(false)));
        }

        if let TokenKind::Identifier(name) = self.peek().kind.clone() {
            self.advance();
            if self.check(&TokenKind::LeftParen) {
                self.advance(); // consume '('
                let mut args = Vec::new();
                if !self.check(&TokenKind::RightParen) {
                    args.push(self.equality()?);
                    while self.check(&TokenKind::Comma) {
                        self.advance();
                        args.push(self.equality()?);
                    }
                }
                if !self.check(&TokenKind::RightParen) {
                    return Err(SapphireError::ParseError {
                        message: "expected ')' after arguments".into(),
                        line: self.peek().line,
                    });
                }
                self.advance(); // consume ')'
                return Ok(Expr::Call { callee: Box::new(Expr::Variable(name)), args });
            }
            if self.check(&TokenKind::Eq) {
                self.advance(); // consume '='
                let value = self.equality()?;
                return Ok(Expr::Assign { name, value: Box::new(value) });
            }
            return Ok(Expr::Variable(name));
        }

        if self.check(&TokenKind::LeftParen) {
            self.advance();
            let expr = self.equality()?;
            self.advance(); // consume ')'
            return Ok(Expr::Grouping(Box::new(expr)));
        }

        Err(SapphireError::ParseError {
            message: format!("unexpected token '{:?}'", self.peek().kind),
            line: self.peek().line,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Expr, Stmt};
    use crate::lexer::Lexer;

    fn parse_expr(source: &str) -> Expr {
        let tokens = Lexer::new(source).scan_tokens();
        let mut stmts = Parser::new(tokens).parse().unwrap();
        match stmts.remove(0) {
            Stmt::Expression(e) => e,
            _ => panic!("expected expression statement"),
        }
    }

    #[test]
    fn test_literal() {
        assert!(matches!(parse_expr("42"), Expr::Literal(Value::Int(42))));
    }

    #[test]
    fn test_addition() {
        assert!(matches!(parse_expr("1+2"), Expr::Binary { .. }));
    }

    #[test]
    fn test_precedence() {
        let expr = parse_expr("1+2*3");
        if let Expr::Binary { op, right, .. } = expr {
            assert_eq!(op.kind, TokenKind::Plus);
            assert!(matches!(*right, Expr::Binary { .. }));
        } else {
            panic!("expected Binary");
        }
    }

    #[test]
    fn test_grouping() {
        let expr = parse_expr("(1+2)*3");
        if let Expr::Binary { op, left, .. } = expr {
            assert_eq!(op.kind, TokenKind::Star);
            assert!(matches!(*left, Expr::Grouping(_)));
        } else {
            panic!("expected Binary");
        }
    }

    #[test]
    fn test_parse_error() {
        let tokens = Lexer::new("1+").scan_tokens();
        assert!(Parser::new(tokens).parse().is_err());
    }

    #[test]
    fn test_print_statement() {
        let tokens = Lexer::new("print 42").scan_tokens();
        let mut stmts = Parser::new(tokens).parse().unwrap();
        assert!(matches!(stmts.remove(0), Stmt::Print(Expr::Literal(Value::Bool(_) | Value::Int(_)))));
    }

    #[test]
    fn test_multiple_statements() {
        let tokens = Lexer::new("x = 1; x + 2").scan_tokens();
        let stmts = Parser::new(tokens).parse().unwrap();
        assert_eq!(stmts.len(), 2);
    }
}
