use crate::ast::{Block, CallArg, Expr, FieldDef, MethodDef, Stmt, StringPart};
use crate::error::SapphireError;
use crate::lexer::Lexer;
use crate::token::{Token, TokenKind};
use crate::value::Value;

pub struct Parser {
    tokens: Vec<Token>,
    current: usize,
    allow_trailing_block: bool,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, current: 0, allow_trailing_block: true }
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

    fn skip_terminators(&mut self) {
        while self.check(&TokenKind::Newline) || self.check(&TokenKind::Semicolon) {
            self.advance();
        }
    }

    pub fn parse(&mut self) -> Result<Vec<Stmt>, SapphireError> {
        let mut stmts = Vec::new();
        loop {
            self.skip_terminators();
            if self.is_at_end() { break; }
            stmts.push(self.statement()?);
        }
        Ok(stmts)
    }

    fn statement(&mut self) -> Result<Stmt, SapphireError> {
        let stmt = self.statement_inner()?;
        // Trailing conditional: `expr if condition`
        if self.check(&TokenKind::If) {
            self.advance();
            self.allow_trailing_block = false;
            let condition = self.logical()?;
            self.allow_trailing_block = true;
            return Ok(Stmt::If { condition, then_branch: vec![stmt], else_branch: None });
        }
        Ok(stmt)
    }

    fn statement_inner(&mut self) -> Result<Stmt, SapphireError> {
        if self.check(&TokenKind::Return) {
            self.advance();
            return Ok(Stmt::Return(self.logical()?));
        }
        if self.check(&TokenKind::Break) {
            self.advance();
            let val = if self.check(&TokenKind::Newline) || self.check(&TokenKind::Semicolon)
                || self.check(&TokenKind::If) || self.is_at_end() {
                Expr::Literal(Value::Nil)
            } else {
                self.logical()?
            };
            return Ok(Stmt::Break(val));
        }
        if self.check(&TokenKind::Next) {
            self.advance();
            let val = if self.check(&TokenKind::Newline) || self.check(&TokenKind::Semicolon)
                || self.check(&TokenKind::If) || self.is_at_end() {
                Expr::Literal(Value::Nil)
            } else {
                self.logical()?
            };
            return Ok(Stmt::Next(val));
        }
        if self.check(&TokenKind::Class) {
            return self.class_def();
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
        // Multi-assignment: ident, ident, ... = expr, expr, ...
        if matches!(self.peek().kind, TokenKind::Identifier(_))
            && self.current + 1 < self.tokens.len()
            && self.tokens[self.current + 1].kind == TokenKind::Comma
        {
            return self.multi_assign();
        }
        if self.check(&TokenKind::Raise) {
            self.advance();
            return Ok(Stmt::Raise(self.logical()?));
        }
        if self.check(&TokenKind::Begin) {
            return self.begin_statement();
        }
        if self.check(&TokenKind::Print) {
            self.advance();
            return Ok(Stmt::Print(self.logical()?));
        }
        Ok(Stmt::Expression(self.logical()?))
    }

    fn class_def(&mut self) -> Result<Stmt, SapphireError> {
        self.advance(); // consume 'class'
        let name = match self.peek().kind.clone() {
            TokenKind::Identifier(n) => { self.advance(); n }
            _ => return Err(SapphireError::ParseError {
                message: "expected class name".into(),
                line: self.peek().line,
            }),
        };
        let superclass = if self.check(&TokenKind::Less) {
            self.advance(); // consume '<'
            match self.peek().kind.clone() {
                TokenKind::Identifier(n) => { self.advance(); Some(n) }
                _ => return Err(SapphireError::ParseError {
                    message: "expected superclass name after '<'".into(),
                    line: self.peek().line,
                }),
            }
        } else {
            None
        };
        if !self.check(&TokenKind::LeftBrace) {
            return Err(SapphireError::ParseError {
                message: "expected '{' after class name".into(),
                line: self.peek().line,
            });
        }
        self.advance(); // consume '{'
        let mut fields = Vec::new();
        let mut methods = Vec::new();
        loop {
            self.skip_terminators();
            if self.check(&TokenKind::RightBrace) || self.is_at_end() { break; }
            if self.check(&TokenKind::Attr) {
                self.advance(); // consume 'attr'
                let field_name = match self.peek().kind.clone() {
                    TokenKind::Identifier(n) => { self.advance(); n }
                    _ => return Err(SapphireError::ParseError {
                        message: "expected field name after 'attr'".into(),
                        line: self.peek().line,
                    }),
                };
                let type_name = if self.check(&TokenKind::Colon) {
                    self.advance();
                    match self.peek().kind.clone() {
                        TokenKind::Identifier(t) => { self.advance(); Some(t) }
                        _ => return Err(SapphireError::ParseError {
                            message: "expected type name after ':'".into(),
                            line: self.peek().line,
                        }),
                    }
                } else {
                    None
                };
                let default = if self.check(&TokenKind::Eq) {
                    self.advance();
                    Some(self.logical()?)
                } else {
                    None
                };
                fields.push(FieldDef { name: field_name, type_name, default });
            } else if self.check(&TokenKind::Def) {
                methods.push(self.method_def()?);
            } else {
                return Err(SapphireError::ParseError {
                    message: "expected 'attr' or 'def' in class body".into(),
                    line: self.peek().line,
                });
            }
        }
        if !self.check(&TokenKind::RightBrace) {
            return Err(SapphireError::ParseError {
                message: "expected '}'".into(),
                line: self.peek().line,
            });
        }
        self.advance(); // consume '}'
        Ok(Stmt::Class { name, superclass, fields, methods })
    }

    fn if_statement(&mut self) -> Result<Stmt, SapphireError> {
        self.advance(); // consume 'if'
        self.allow_trailing_block = false;
        let condition = self.logical()?;
        self.allow_trailing_block = true;
        let then_branch = self.block()?;
        let else_branch = if self.check(&TokenKind::Else) {
            self.advance();
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

    fn method_def(&mut self) -> Result<MethodDef, SapphireError> {
        self.advance(); // consume 'def'
        let name = match self.peek().kind.clone() {
            TokenKind::Identifier(n) => { self.advance(); n }
            _ => return Err(SapphireError::ParseError {
                message: "expected method name".into(),
                line: self.peek().line,
            }),
        };
        if !self.check(&TokenKind::LeftParen) {
            return Err(SapphireError::ParseError {
                message: "expected '(' after method name".into(),
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
        Ok(MethodDef { name, params, body })
    }

    fn while_statement(&mut self) -> Result<Stmt, SapphireError> {
        self.advance(); // consume 'while'
        self.allow_trailing_block = false;
        let condition = self.logical()?;
        self.allow_trailing_block = true;
        let body = self.block()?;
        Ok(Stmt::While { condition, body })
    }

    fn multi_assign(&mut self) -> Result<Stmt, SapphireError> {
        let mut names = Vec::new();
        loop {
            match self.peek().kind.clone() {
                TokenKind::Identifier(n) => { self.advance(); names.push(n); }
                _ => return Err(SapphireError::ParseError {
                    message: "expected identifier in multiple assignment".into(),
                    line: self.peek().line,
                }),
            }
            if self.check(&TokenKind::Comma) {
                self.advance();
            } else {
                break;
            }
        }
        if !self.check(&TokenKind::Eq) {
            return Err(SapphireError::ParseError {
                message: "expected '=' in multiple assignment".into(),
                line: self.peek().line,
            });
        }
        self.advance(); // consume '='
        let mut values = Vec::new();
        loop {
            values.push(self.logical()?);
            if self.check(&TokenKind::Comma) {
                self.advance();
            } else {
                break;
            }
        }
        Ok(Stmt::MultiAssign { names, values })
    }

    fn begin_statement(&mut self) -> Result<Stmt, SapphireError> {
        self.advance(); // consume 'begin'
        let mut body = Vec::new();
        loop {
            self.skip_terminators();
            if self.check(&TokenKind::Rescue) || self.check(&TokenKind::End) || self.is_at_end() { break; }
            body.push(self.statement()?);
        }
        let (rescue_var, rescue_body) = if self.check(&TokenKind::Rescue) {
            self.advance(); // consume 'rescue'
            let var = if let TokenKind::Identifier(n) = self.peek().kind.clone() {
                self.advance();
                Some(n)
            } else {
                None
            };
            let mut rescue_body = Vec::new();
            loop {
                self.skip_terminators();
                if self.check(&TokenKind::End) || self.is_at_end() { break; }
                rescue_body.push(self.statement()?);
            }
            (var, rescue_body)
        } else {
            (None, Vec::new())
        };
        if !self.check(&TokenKind::End) {
            return Err(SapphireError::ParseError {
                message: "expected 'end' to close 'begin'".into(),
                line: self.peek().line,
            });
        }
        self.advance(); // consume 'end'
        Ok(Stmt::Begin { body, rescue_var, rescue_body })
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
        loop {
            self.skip_terminators();
            if self.check(&TokenKind::RightBrace) || self.is_at_end() { break; }
            stmts.push(self.statement()?);
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

    // logical: range (('&&' | '||') range)*
    fn logical(&mut self) -> Result<Expr, SapphireError> {
        let mut left = self.range()?;
        while self.check(&TokenKind::AmpAmp) || self.check(&TokenKind::PipePipe) {
            let op = self.advance().clone();
            let right = self.range()?;
            left = Expr::Binary { left: Box::new(left), op, right: Box::new(right) };
        }
        Ok(left)
    }

    // range: equality ('..' equality)?
    fn range(&mut self) -> Result<Expr, SapphireError> {
        let left = self.equality()?;
        if self.check(&TokenKind::DotDot) {
            self.advance();
            let right = self.equality()?;
            return Ok(Expr::Range { from: Box::new(left), to: Box::new(right) });
        }
        Ok(left)
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
            left = Expr::Binary { left: Box::new(left), op, right: Box::new(right) };
        }
        Ok(left)
    }

    // factor: unary (('*' | '/') unary)*
    fn factor(&mut self) -> Result<Expr, SapphireError> {
        let mut left = self.unary()?;
        while self.check(&TokenKind::Star) || self.check(&TokenKind::Slash) || self.check(&TokenKind::Percent) {
            let op = self.advance().clone();
            let right = self.unary()?;
            left = Expr::Binary { left: Box::new(left), op, right: Box::new(right) };
        }
        Ok(left)
    }

    // unary: ('!' | '-') unary | call
    fn unary(&mut self) -> Result<Expr, SapphireError> {
        if self.check(&TokenKind::Bang) || self.check(&TokenKind::Minus) {
            let op = self.advance().clone();
            let right = self.unary()?;
            return Ok(Expr::Unary { op, right: Box::new(right) });
        }
        self.call()
    }

    // call: primary ('(' args ')' | '.' IDENTIFIER)*
    fn call(&mut self) -> Result<Expr, SapphireError> {
        let mut expr = self.primary()?;
        loop {
            if self.check(&TokenKind::LeftParen) {
                expr = self.finish_call(expr)?;
            } else if self.check(&TokenKind::Dot) {
                self.advance(); // consume '.'
                let name = match self.peek().kind.clone() {
                    TokenKind::Identifier(n) => { self.advance(); n }
                    _ => return Err(SapphireError::ParseError {
                        message: "expected field or method name after '.'".into(),
                        line: self.peek().line,
                    }),
                };
                if self.check(&TokenKind::Eq) {
                    self.advance(); // consume '='
                    let value = self.logical()?;
                    expr = Expr::Set { object: Box::new(expr), name, value: Box::new(value) };
                    break;
                }
                if self.allow_trailing_block && self.check(&TokenKind::LeftBrace) {
                    let block = self.parse_block()?;
                    let get = Expr::Get { object: Box::new(expr), name };
                    expr = Expr::Call { callee: Box::new(get), args: Vec::new(), block };
                    break;
                }
                expr = Expr::Get { object: Box::new(expr), name };
            } else if self.check(&TokenKind::AmpDot) {
                self.advance(); // consume '&.'
                let name = match self.peek().kind.clone() {
                    TokenKind::Identifier(n) => { self.advance(); n }
                    _ => return Err(SapphireError::ParseError {
                        message: "expected method or field name after '&.'".into(),
                        line: self.peek().line,
                    }),
                };
                if self.check(&TokenKind::LeftParen) {
                    let safe_get = Expr::SafeGet { object: Box::new(expr), name };
                    let call = self.finish_call(safe_get)?;
                    expr = call;
                } else {
                    expr = Expr::SafeGet { object: Box::new(expr), name };
                }
            } else if self.check(&TokenKind::LeftBracket) {
                self.advance(); // consume '['
                let index = self.logical()?;
                if !self.check(&TokenKind::RightBracket) {
                    return Err(SapphireError::ParseError {
                        message: "expected ']' after index".into(),
                        line: self.peek().line,
                    });
                }
                self.advance(); // consume ']'
                if self.check(&TokenKind::Eq) {
                    self.advance(); // consume '='
                    let value = self.logical()?;
                    expr = Expr::IndexSet { object: Box::new(expr), index: Box::new(index), value: Box::new(value) };
                    break;
                }
                expr = Expr::Index { object: Box::new(expr), index: Box::new(index) };
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn finish_call(&mut self, callee: Expr) -> Result<Expr, SapphireError> {
        self.advance(); // consume '('
        let mut args = Vec::new();
        if !self.check(&TokenKind::RightParen) {
            args.push(self.parse_arg()?);
            while self.check(&TokenKind::Comma) {
                self.advance();
                args.push(self.parse_arg()?);
            }
        }
        if !self.check(&TokenKind::RightParen) {
            return Err(SapphireError::ParseError {
                message: "expected ')' after arguments".into(),
                line: self.peek().line,
            });
        }
        self.advance(); // consume ')'
        let block = self.parse_block()?;
        Ok(Expr::Call { callee: Box::new(callee), args, block })
    }

    fn parse_block(&mut self) -> Result<Option<Block>, SapphireError> {
        if !self.check(&TokenKind::LeftBrace) {
            return Ok(None);
        }
        self.advance(); // consume '{'
        let params = if self.check(&TokenKind::Pipe) {
            self.advance(); // consume '|'
            let mut params = Vec::new();
            if !self.check(&TokenKind::Pipe) {
                loop {
                    match self.peek().kind.clone() {
                        TokenKind::Identifier(n) => { self.advance(); params.push(n); }
                        _ => return Err(SapphireError::ParseError {
                            message: "expected parameter name in block".into(),
                            line: self.peek().line,
                        }),
                    }
                    if !self.check(&TokenKind::Comma) { break; }
                    self.advance(); // consume ','
                }
            }
            if !self.check(&TokenKind::Pipe) {
                return Err(SapphireError::ParseError {
                    message: "expected '|' after block parameters".into(),
                    line: self.peek().line,
                });
            }
            self.advance(); // consume '|'
            params
        } else {
            Vec::new()
        };
        let mut body = Vec::new();
        loop {
            self.skip_terminators();
            if self.check(&TokenKind::RightBrace) || self.is_at_end() { break; }
            body.push(self.statement()?);
        }
        if !self.check(&TokenKind::RightBrace) {
            return Err(SapphireError::ParseError {
                message: "expected '}'".into(),
                line: self.peek().line,
            });
        }
        self.advance(); // consume '}'
        Ok(Some(Block { params, body }))
    }

    fn parse_arg(&mut self) -> Result<CallArg, SapphireError> {
        // Named arg: identifier ':' expr
        if let TokenKind::Identifier(name) = self.peek().kind.clone() {
            if self.current + 1 < self.tokens.len()
                && self.tokens[self.current + 1].kind == TokenKind::Colon
            {
                self.advance(); // consume identifier
                self.advance(); // consume ':'
                return Ok(CallArg { name: Some(name), value: self.logical()? });
            }
        }
        Ok(CallArg { name: None, value: self.logical()? })
    }

    // primary: NUMBER | STRING | BOOL | IDENTIFIER ('=' equality)? | '(' equality ')'
    fn primary(&mut self) -> Result<Expr, SapphireError> {
        if let TokenKind::Number(n) = self.peek().kind {
            self.advance();
            return Ok(Expr::Literal(Value::Int(n)));
        }

        if let TokenKind::Float(f) = self.peek().kind {
            self.advance();
            return Ok(Expr::Literal(Value::Float(f)));
        }

        if let TokenKind::StringLit(s) = self.peek().kind.clone() {
            self.advance();
            return Ok(Expr::Literal(Value::Str(s)));
        }

        if let TokenKind::StringInterp(raw_parts) = self.peek().kind.clone() {
            self.advance();
            let mut parts = Vec::new();
            for (content, is_expr) in raw_parts {
                if is_expr {
                    let tokens = Lexer::new(&content).scan_tokens();
                    let expr = Parser::new(tokens).logical()?;
                    parts.push(StringPart::Expr(Box::new(expr)));
                } else {
                    parts.push(StringPart::Lit(content));
                }
            }
            return Ok(Expr::StringInterp(parts));
        }

        if self.check(&TokenKind::True) {
            self.advance();
            return Ok(Expr::Literal(Value::Bool(true)));
        }

        if self.check(&TokenKind::False) {
            self.advance();
            return Ok(Expr::Literal(Value::Bool(false)));
        }

        if self.check(&TokenKind::Nil) {
            self.advance();
            return Ok(Expr::Literal(Value::Nil));
        }

        if self.check(&TokenKind::SelfKw) {
            self.advance();
            return Ok(Expr::SelfExpr);
        }

        if self.check(&TokenKind::Yield) {
            self.advance();
            let args = if self.check(&TokenKind::LeftParen) {
                self.advance(); // consume '('
                let mut args = Vec::new();
                if !self.check(&TokenKind::RightParen) {
                    args.push(self.parse_arg()?);
                    while self.check(&TokenKind::Comma) {
                        self.advance();
                        args.push(self.parse_arg()?);
                    }
                }
                if !self.check(&TokenKind::RightParen) {
                    return Err(SapphireError::ParseError {
                        message: "expected ')' after yield arguments".into(),
                        line: self.peek().line,
                    });
                }
                self.advance(); // consume ')'
                args
            } else {
                Vec::new()
            };
            return Ok(Expr::Yield { args });
        }

        if self.check(&TokenKind::SuperKw) {
            self.advance();
            // Expect super.method_name(args)
            if !self.check(&TokenKind::Dot) {
                return Err(SapphireError::ParseError {
                    message: "expected '.' after 'super'".into(),
                    line: self.peek().line,
                });
            }
            self.advance(); // consume '.'
            let method = if let TokenKind::Identifier(name) = self.peek().kind.clone() {
                self.advance();
                name
            } else {
                return Err(SapphireError::ParseError {
                    message: "expected method name after 'super.'".into(),
                    line: self.peek().line,
                });
            };
            let (args, block) = if self.check(&TokenKind::LeftParen) {
                self.advance(); // consume '('
                let mut args = Vec::new();
                if !self.check(&TokenKind::RightParen) {
                    args.push(self.parse_arg()?);
                    while self.check(&TokenKind::Comma) {
                        self.advance();
                        args.push(self.parse_arg()?);
                    }
                }
                if !self.check(&TokenKind::RightParen) {
                    return Err(SapphireError::ParseError {
                        message: "expected ')' after arguments".into(),
                        line: self.peek().line,
                    });
                }
                self.advance(); // consume ')'
                let block = self.parse_block()?;
                (args, block)
            } else {
                (Vec::new(), None)
            };
            return Ok(Expr::Super { method, args, block });
        }

        if let TokenKind::Identifier(name) = self.peek().kind.clone() {
            self.advance();
            if self.check(&TokenKind::Eq) {
                self.advance(); // consume '='
                let value = self.logical()?;
                return Ok(Expr::Assign { name, value: Box::new(value) });
            }
            return Ok(Expr::Variable(name));
        }

        if self.check(&TokenKind::LeftParen) {
            self.advance();
            let expr = self.logical()?;
            self.advance(); // consume ')'
            return Ok(Expr::Grouping(Box::new(expr)));
        }

        if self.check(&TokenKind::LeftBracket) {
            self.advance(); // consume '['
            let mut elements = Vec::new();
            if !self.check(&TokenKind::RightBracket) {
                elements.push(self.logical()?);
                while self.check(&TokenKind::Comma) {
                    self.advance();
                    elements.push(self.logical()?);
                }
            }
            if !self.check(&TokenKind::RightBracket) {
                return Err(SapphireError::ParseError {
                    message: "expected ']' after list elements".into(),
                    line: self.peek().line,
                });
            }
            self.advance(); // consume ']'
            return Ok(Expr::ListLit(elements));
        }

        if self.check(&TokenKind::LeftBrace) {
            self.advance(); // consume '{'
            let mut pairs = Vec::new();
            if !self.check(&TokenKind::RightBrace) {
                loop {
                    let key = match self.peek().kind.clone() {
                        TokenKind::Identifier(k) => { self.advance(); k }
                        _ => return Err(SapphireError::ParseError {
                            message: "expected key name in map literal".into(),
                            line: self.peek().line,
                        }),
                    };
                    if !self.check(&TokenKind::Colon) {
                        return Err(SapphireError::ParseError {
                            message: "expected ':' after map key".into(),
                            line: self.peek().line,
                        });
                    }
                    self.advance(); // consume ':'
                    let value = self.logical()?;
                    pairs.push((key, value));
                    if !self.check(&TokenKind::Comma) { break; }
                    self.advance(); // consume ','
                }
            }
            if !self.check(&TokenKind::RightBrace) {
                return Err(SapphireError::ParseError {
                    message: "expected '}' after map literal".into(),
                    line: self.peek().line,
                });
            }
            self.advance(); // consume '}'
            return Ok(Expr::MapLit(pairs));
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

    #[test]
    fn test_class_def() {
        let tokens = Lexer::new("class Point { attr x: Int; attr y: Int }").scan_tokens();
        let mut stmts = Parser::new(tokens).parse().unwrap();
        assert!(matches!(stmts.remove(0), Stmt::Class { name, .. } if name == "Point"));
    }

    #[test]
    fn test_field_access() {
        let expr = parse_expr("p.x");
        assert!(matches!(expr, Expr::Get { name, .. } if name == "x"));
    }

    #[test]
    fn test_named_arg_call() {
        let expr = parse_expr("Point.new(x: 1, y: 2)");
        assert!(matches!(expr, Expr::Call { .. }));
    }
}
