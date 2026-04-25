use crate::ast::{Block, CallArg, Expr, FieldDef, MethodDef, ParamDef, StringPart, TypeExpr};
use crate::error::SapphireError;
use crate::lexer::Lexer;
use crate::token::{Token, TokenKind};
use crate::value::Value;

/// Return the display name for a type expression (used in error messages).
fn type_expr_display_name(te: &TypeExpr) -> String {
    match te {
        TypeExpr::Named(n) => n.clone(),
        TypeExpr::Union(arms) => arms.iter().map(type_expr_display_name).collect::<Vec<_>>().join(" | "),
        TypeExpr::Any => "Any".to_string(),
    }
}

pub struct Parser {
    tokens: Vec<Token>,
    current: usize,
    allow_trailing_block: bool,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            current: 0,
            allow_trailing_block: true,
        }
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.current]
    }

    // Returns true if the current '{' starts a block rather than a map literal.
    // { |   → block with params
    // { }   → empty map
    // { id: → map entry
    // other → block without params
    fn is_block_start(&self) -> bool {
        if !self.check(&TokenKind::LeftBrace) {
            return false;
        }
        match self.tokens.get(self.current + 1).map(|t| &t.kind) {
            Some(TokenKind::Pipe) => true,
            Some(TokenKind::RightBrace) => false,
            Some(TokenKind::Identifier(_)) => !matches!(
                self.tokens.get(self.current + 2).map(|t| &t.kind),
                Some(TokenKind::Colon)
            ),
            _ => true,
        }
    }

    fn parse_type_ann(&mut self) -> Result<Option<TypeExpr>, SapphireError> {
        if !self.check(&TokenKind::Colon) {
            return Ok(None);
        }
        self.advance(); // consume ':'
        Ok(Some(self.parse_type_expr()?))
    }

    fn parse_return_type(&mut self) -> Result<Option<TypeExpr>, SapphireError> {
        if !self.check(&TokenKind::Arrow) {
            return Ok(None);
        }
        self.advance(); // consume '->'
        Ok(Some(self.parse_type_expr()?))
    }

    /// Parse a type expression, including unions (`Int | String`) and multiline forms.
    /// Optional leading `|` is allowed for alignment style.
    fn parse_type_expr(&mut self) -> Result<TypeExpr, SapphireError> {
        self.skip_terminators(); // allow newline before first arm
        // Optional leading '|' for multiline alignment style: `| Int | String`
        if self.check(&TokenKind::Pipe) {
            self.advance();
            self.skip_terminators();
        }

        let first = self.parse_single_type()?;

        // Check for additional union arms (may span newlines)
        self.skip_terminators();
        if !self.check(&TokenKind::Pipe) {
            return Ok(first);
        }

        let mut arms = vec![first];
        while self.check(&TokenKind::Pipe) {
            self.advance();
            self.skip_terminators();
            arms.push(self.parse_single_type()?);
            self.skip_terminators();
        }

        // Flatten nested unions from T? arms (T? desugars to Union([T, Nil]))
        let mut flat: Vec<TypeExpr> = Vec::new();
        for arm in arms {
            match arm {
                TypeExpr::Union(inner) => flat.extend(inner),
                other => flat.push(other),
            }
        }

        // Any absorbs everything
        if flat.iter().any(|t| matches!(t, TypeExpr::Any)) {
            return Ok(TypeExpr::Any);
        }

        // Nil is not allowed as an explicit union arm — force ? syntax
        if flat.iter().any(|t| matches!(t, TypeExpr::Named(n) if n == "Nil")) {
            let non_nil_names: Vec<String> = flat
                .iter()
                .filter(|t| !matches!(t, TypeExpr::Named(n) if n == "Nil"))
                .map(type_expr_display_name)
                .collect();
            let suggestion = if non_nil_names.len() == 1 {
                format!("{}?", non_nil_names[0])
            } else {
                format!("({})?", non_nil_names.join(" | "))
            };
            return Err(SapphireError::ParseError {
                message: format!(
                    "Nil is not allowed as a union arm; use {} instead",
                    suggestion
                ),
                line: self.peek().line,
            });
        }

        Ok(TypeExpr::Union(flat))
    }

    /// Parse a single type: a named type (with optional `?` suffix), or a parenthesized group.
    fn parse_single_type(&mut self) -> Result<TypeExpr, SapphireError> {
        // Parenthesized group: (Int | String)?
        if self.check(&TokenKind::LeftParen) {
            self.advance();
            let inner = self.parse_type_expr()?;
            if !self.check(&TokenKind::RightParen) {
                return Err(SapphireError::ParseError {
                    message: "expected ')' after type group".into(),
                    line: self.peek().line,
                });
            }
            self.advance(); // consume ')'
            if self.check(&TokenKind::Question) {
                self.advance(); // consume '?'
                return Ok(match inner {
                    TypeExpr::Union(mut arms) => {
                        arms.push(TypeExpr::Named("Nil".to_string()));
                        TypeExpr::Union(arms)
                    }
                    other => TypeExpr::Union(vec![other, TypeExpr::Named("Nil".to_string())]),
                });
            }
            return Ok(inner);
        }

        match self.peek().kind.clone() {
            TokenKind::Identifier(t) => {
                self.advance();
                // T? is sugar for T | Nil
                if t.ends_with('?') {
                    let base = t[..t.len() - 1].to_string();
                    return Ok(TypeExpr::Union(vec![
                        TypeExpr::Named(base),
                        TypeExpr::Named("Nil".to_string()),
                    ]));
                }
                Ok(TypeExpr::Named(t))
            }
            TokenKind::Nil => {
                self.advance();
                Ok(TypeExpr::Named("Nil".to_string()))
            }
            _ => Err(SapphireError::ParseError {
                message: "expected type name".into(),
                line: self.peek().line,
            }),
        }
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

    // Returns true if the next non-newline token is a '.', used to allow method
    // chaining across lines after a block: `.map { |n| n * 2 }\n  .each { ... }`.
    fn next_non_newline_is_dot(&self) -> bool {
        let mut i = self.current;
        while i < self.tokens.len() {
            match &self.tokens[i].kind {
                TokenKind::Newline | TokenKind::Semicolon => i += 1,
                TokenKind::Dot => return true,
                _ => return false,
            }
        }
        false
    }

    pub fn parse(&mut self) -> Result<Vec<Expr>, SapphireError> {
        let mut exprs = Vec::new();
        loop {
            self.skip_terminators();
            if self.is_at_end() {
                break;
            }
            exprs.push(self.statement()?);
        }
        Ok(exprs)
    }

    fn statement(&mut self) -> Result<Expr, SapphireError> {
        let stmt = self.statement_inner()?;
        // Trailing conditional: `expr if condition`
        if self.check(&TokenKind::If) {
            self.advance();
            self.allow_trailing_block = false;
            let condition = self.logical()?;
            self.allow_trailing_block = true;
            return Ok(Expr::If {
                condition: Box::new(condition),
                then_branch: vec![stmt],
                else_branch: None,
            });
        }
        Ok(stmt)
    }

    fn statement_inner(&mut self) -> Result<Expr, SapphireError> {
        if self.check(&TokenKind::Return) {
            self.advance();
            return Ok(Expr::Return(Box::new(self.logical()?)));
        }
        if self.check(&TokenKind::Break) {
            self.advance();
            let val = if self.check(&TokenKind::Newline)
                || self.check(&TokenKind::Semicolon)
                || self.check(&TokenKind::If)
                || self.check(&TokenKind::RightBrace)
                || self.is_at_end()
            {
                Expr::Literal(Value::Nil)
            } else {
                self.logical()?
            };
            return Ok(Expr::Break(Box::new(val)));
        }
        if self.check(&TokenKind::Next) {
            self.advance();
            let val = if self.check(&TokenKind::Newline)
                || self.check(&TokenKind::Semicolon)
                || self.check(&TokenKind::If)
                || self.check(&TokenKind::RightBrace)
                || self.is_at_end()
            {
                Expr::Literal(Value::Nil)
            } else {
                self.logical()?
            };
            return Ok(Expr::Next(Box::new(val)));
        }
        if self.check(&TokenKind::Class) {
            return self.class_def();
        }
        if self.check(&TokenKind::Def) {
            return self.function_def();
        }
        if self.check(&TokenKind::If) {
            return self.if_expr();
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
            return Ok(Expr::Raise(Box::new(self.logical()?)));
        }
        if self.check(&TokenKind::Begin) {
            return self.begin_expr();
        }
        if self.check(&TokenKind::Print) {
            self.advance();
            return Ok(Expr::Print(Box::new(self.logical()?)));
        }
        if self.check(&TokenKind::Import) {
            let line = self.peek().line;
            self.advance();
            match self.peek().kind.clone() {
                TokenKind::StringLit(path) => {
                    if !path.starts_with("./") && !path.starts_with("../") {
                        return Err(SapphireError::ParseError {
                            message: format!(
                                "import path must be relative (start with ./ or ../): {:?}",
                                path
                            ),
                            line,
                        });
                    }
                    self.advance();
                    return Ok(Expr::Import { path });
                }
                _ => {
                    return Err(SapphireError::ParseError {
                        message: "expected a string literal after 'import'".into(),
                        line,
                    });
                }
            }
        }
        self.logical()
    }

    fn class_def(&mut self) -> Result<Expr, SapphireError> {
        self.advance(); // consume 'class'
        let name = match self.peek().kind.clone() {
            TokenKind::Identifier(n) => {
                self.advance();
                n
            }
            _ => {
                return Err(SapphireError::ParseError {
                    message: "expected class name".into(),
                    line: self.peek().line,
                });
            }
        };
        let superclass = if self.check(&TokenKind::Less) {
            self.advance(); // consume '<'
            let first = match self.peek().kind.clone() {
                TokenKind::Identifier(n) => {
                    self.advance();
                    n
                }
                _ => {
                    return Err(SapphireError::ParseError {
                        message: "expected superclass name after '<'".into(),
                        line: self.peek().line,
                    });
                }
            };
            let mut expr: Expr = Expr::Variable(first);
            while self.check(&TokenKind::Dot) {
                self.advance(); // consume '.'
                let field = match self.peek().kind.clone() {
                    TokenKind::Identifier(n) => {
                        self.advance();
                        n
                    }
                    _ => {
                        return Err(SapphireError::ParseError {
                            message: "expected identifier after '.' in superclass".into(),
                            line: self.peek().line,
                        });
                    }
                };
                expr = Expr::Get {
                    object: Box::new(expr),
                    name: field,
                };
            }
            Some(Box::new(expr))
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
        let mut nested = Vec::new();
        let mut constants = Vec::new();
        loop {
            self.skip_terminators();
            if self.check(&TokenKind::RightBrace) || self.is_at_end() {
                break;
            }
            if self.check(&TokenKind::Class) {
                nested.push(self.class_def()?);
            } else if let TokenKind::Identifier(n) = self.peek().kind.clone() {
                // ALL_CAPS identifier followed by `=` is a class constant: `PI = 3.14`
                let next_is_eq = self
                    .tokens
                    .get(self.current + 1)
                    .map(|t| t.kind == TokenKind::Eq)
                    .unwrap_or(false);
                if n.chars()
                    .all(|c| c.is_uppercase() || c == '_' || c.is_ascii_digit())
                    && next_is_eq
                {
                    self.advance(); // consume name
                    self.advance(); // consume '='
                    let val = self.logical()?;
                    constants.push((n, Box::new(val)));
                } else {
                    return Err(SapphireError::ParseError {
                        message: "expected 'attr', 'class', 'def', 'defp', or 'self' in class body"
                            .into(),
                        line: self.peek().line,
                    });
                }
            } else if self.check(&TokenKind::Attr) {
                self.advance(); // consume 'attr'
                let field_name = match self.peek().kind.clone() {
                    TokenKind::Identifier(n) => {
                        self.advance();
                        n
                    }
                    _ => {
                        return Err(SapphireError::ParseError {
                            message: "expected field name after 'attr'".into(),
                            line: self.peek().line,
                        });
                    }
                };
                let type_ann = self.parse_type_ann()?;
                let default = if self.check(&TokenKind::Eq) {
                    self.advance();
                    Some(self.logical()?)
                } else {
                    None
                };
                fields.push(FieldDef {
                    name: field_name,
                    type_ann,
                    default,
                });
            } else if self.check(&TokenKind::Def) || self.check(&TokenKind::Defp) {
                let private = self.check(&TokenKind::Defp);
                methods.push(self.method_def(private)?);
            } else if self.check(&TokenKind::SelfKw) {
                self.advance(); // consume `self`
                if !self.check(&TokenKind::LeftBrace) {
                    return Err(SapphireError::ParseError {
                        message: "expected '{' after 'self' in class body".into(),
                        line: self.peek().line,
                    });
                }
                self.advance(); // consume '{'
                loop {
                    self.skip_terminators();
                    if self.check(&TokenKind::RightBrace) || self.is_at_end() {
                        break;
                    }
                    if self.check(&TokenKind::Def) || self.check(&TokenKind::Defp) {
                        let private = self.check(&TokenKind::Defp);
                        let mut m = self.method_def(private)?;
                        m.class_method = true;
                        methods.push(m);
                    } else {
                        return Err(SapphireError::ParseError {
                            message: "expected 'def' or 'defp' inside 'self' block".into(),
                            line: self.peek().line,
                        });
                    }
                }
                if !self.check(&TokenKind::RightBrace) {
                    return Err(SapphireError::ParseError {
                        message: "expected '}' to close 'self' block".into(),
                        line: self.peek().line,
                    });
                }
                self.advance(); // consume '}'
            } else {
                return Err(SapphireError::ParseError {
                    message: "expected 'attr', 'class', 'def', 'defp', or 'self' in class body"
                        .into(),
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
        Ok(Expr::Class {
            name,
            superclass,
            fields,
            methods,
            nested,
            constants,
        })
    }

    fn if_expr(&mut self) -> Result<Expr, SapphireError> {
        self.advance(); // consume 'if'
        self.allow_trailing_block = false;
        let condition = self.logical()?;
        self.allow_trailing_block = true;
        let then_branch = self.block()?;
        self.skip_terminators(); // allow elsif/else on the next line
        let else_branch = if self.check(&TokenKind::Elsif) {
            Some(vec![self.elsif_chain()?])
        } else if self.check(&TokenKind::Else) {
            self.advance();
            Some(self.block()?)
        } else {
            None
        };
        Ok(Expr::If {
            condition: Box::new(condition),
            then_branch,
            else_branch,
        })
    }

    fn elsif_chain(&mut self) -> Result<Expr, SapphireError> {
        self.advance(); // consume 'elsif'
        self.allow_trailing_block = false;
        let condition = self.logical()?;
        self.allow_trailing_block = true;
        let then_branch = self.block()?;
        self.skip_terminators(); // allow elsif/else on the next line
        let else_branch = if self.check(&TokenKind::Elsif) {
            Some(vec![self.elsif_chain()?])
        } else if self.check(&TokenKind::Else) {
            self.advance();
            Some(self.block()?)
        } else {
            None
        };
        Ok(Expr::If {
            condition: Box::new(condition),
            then_branch,
            else_branch,
        })
    }

    fn function_def(&mut self) -> Result<Expr, SapphireError> {
        self.advance(); // consume 'def'
        // Anonymous lambda: `def(params) { body }`
        if self.check(&TokenKind::LeftParen) {
            return self.lambda_def();
        }
        let name = match self.peek().kind.clone() {
            TokenKind::Identifier(n) => {
                self.advance();
                n
            }
            _ => {
                return Err(SapphireError::ParseError {
                    message: "expected function name or '(' after 'def'".into(),
                    line: self.peek().line,
                });
            }
        };
        let mut params = Vec::new();
        if self.check(&TokenKind::LeftParen) {
            self.advance(); // consume '('
            if !self.check(&TokenKind::RightParen) {
                loop {
                    match self.peek().kind.clone() {
                        TokenKind::Identifier(p) => {
                            self.advance();
                            let type_ann = self.parse_type_ann()?;
                            params.push(ParamDef { name: p, type_ann });
                        }
                        _ => {
                            return Err(SapphireError::ParseError {
                                message: "expected parameter name".into(),
                                line: self.peek().line,
                            });
                        }
                    }
                    if !self.check(&TokenKind::Comma) {
                        break;
                    }
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
        }
        let return_type = self.parse_return_type()?;
        let body = self.block_with_rescue()?;
        Ok(Expr::Function {
            name,
            params,
            return_type,
            body,
        })
    }

    fn lambda_def(&mut self) -> Result<Expr, SapphireError> {
        // `(` already peeked; `def` already consumed.
        self.advance(); // consume '('
        let mut params = Vec::new();
        if !self.check(&TokenKind::RightParen) {
            loop {
                match self.peek().kind.clone() {
                    TokenKind::Identifier(p) => {
                        self.advance();
                        params.push(p);
                    }
                    _ => {
                        return Err(SapphireError::ParseError {
                            message: "expected parameter name in lambda".into(),
                            line: self.peek().line,
                        });
                    }
                }
                if !self.check(&TokenKind::Comma) {
                    break;
                }
                self.advance();
            }
        }
        if !self.check(&TokenKind::RightParen) {
            return Err(SapphireError::ParseError {
                message: "expected ')' after lambda parameters".into(),
                line: self.peek().line,
            });
        }
        self.advance(); // consume ')'
        let body = self.block()?;
        Ok(Expr::Lambda { params, body })
    }

    fn method_def(&mut self, private: bool) -> Result<MethodDef, SapphireError> {
        self.advance(); // consume 'def' or 'defp'
        let name = match self.peek().kind.clone() {
            TokenKind::Identifier(n) => {
                self.advance();
                n
            }
            _ => {
                return Err(SapphireError::ParseError {
                    message: "expected method name".into(),
                    line: self.peek().line,
                });
            }
        };
        let mut params = Vec::new();
        if self.check(&TokenKind::LeftParen) {
            self.advance(); // consume '('
            if !self.check(&TokenKind::RightParen) {
                loop {
                    match self.peek().kind.clone() {
                        TokenKind::Identifier(p) => {
                            self.advance();
                            let type_ann = self.parse_type_ann()?;
                            params.push(ParamDef { name: p, type_ann });
                        }
                        _ => {
                            return Err(SapphireError::ParseError {
                                message: "expected parameter name".into(),
                                line: self.peek().line,
                            });
                        }
                    }
                    if !self.check(&TokenKind::Comma) {
                        break;
                    }
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
        }
        let return_type = self.parse_return_type()?;
        let body = self.block_with_rescue()?;
        Ok(MethodDef {
            name,
            params,
            return_type,
            body,
            private,
            class_method: false,
        })
    }

    fn while_statement(&mut self) -> Result<Expr, SapphireError> {
        self.advance(); // consume 'while'
        self.allow_trailing_block = false;
        let condition = self.logical()?;
        self.allow_trailing_block = true;
        let body = self.block()?;
        Ok(Expr::While {
            condition: Box::new(condition),
            body,
        })
    }

    fn multi_assign(&mut self) -> Result<Expr, SapphireError> {
        let mut names = Vec::new();
        loop {
            match self.peek().kind.clone() {
                TokenKind::Identifier(n) => {
                    self.advance();
                    names.push(n);
                }
                _ => {
                    return Err(SapphireError::ParseError {
                        message: "expected identifier in multiple assignment".into(),
                        line: self.peek().line,
                    });
                }
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
        Ok(Expr::MultiAssign { names, values })
    }

    fn begin_expr(&mut self) -> Result<Expr, SapphireError> {
        self.advance(); // consume 'begin'
        let mut body = Vec::new();
        loop {
            self.skip_terminators();
            if self.check(&TokenKind::Rescue) || self.check(&TokenKind::End) || self.is_at_end() {
                break;
            }
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
                if self.check(&TokenKind::Else) || self.check(&TokenKind::End) || self.is_at_end() {
                    break;
                }
                rescue_body.push(self.statement()?);
            }
            (var, rescue_body)
        } else {
            (None, Vec::new())
        };
        let else_body = if self.check(&TokenKind::Else) {
            self.advance(); // consume 'else'
            let mut else_body = Vec::new();
            loop {
                self.skip_terminators();
                if self.check(&TokenKind::End) || self.is_at_end() {
                    break;
                }
                else_body.push(self.statement()?);
            }
            else_body
        } else {
            Vec::new()
        };
        if !self.check(&TokenKind::End) {
            return Err(SapphireError::ParseError {
                message: "expected 'end' to close 'begin'".into(),
                line: self.peek().line,
            });
        }
        self.advance(); // consume 'end'
        Ok(Expr::Begin {
            body,
            rescue_var,
            rescue_body,
            else_body,
        })
    }

    // Like block(), but wraps the body in Expr::Begin if a rescue clause is present.
    // Used by function and method definitions.
    fn block_with_rescue(&mut self) -> Result<Vec<Expr>, SapphireError> {
        if !self.check(&TokenKind::LeftBrace) {
            return Err(SapphireError::ParseError {
                message: "expected '{'".into(),
                line: self.peek().line,
            });
        }
        self.advance(); // consume '{'
        let mut body = Vec::new();
        loop {
            self.skip_terminators();
            if self.check(&TokenKind::Rescue)
                || self.check(&TokenKind::RightBrace)
                || self.is_at_end()
            {
                break;
            }
            body.push(self.statement()?);
        }
        if self.check(&TokenKind::Rescue) {
            self.advance(); // consume 'rescue'
            let rescue_var = if let TokenKind::Identifier(n) = self.peek().kind.clone() {
                self.advance();
                Some(n)
            } else {
                None
            };
            let mut rescue_body = Vec::new();
            loop {
                self.skip_terminators();
                if self.check(&TokenKind::RightBrace) || self.is_at_end() {
                    break;
                }
                rescue_body.push(self.statement()?);
            }
            if !self.check(&TokenKind::RightBrace) {
                return Err(SapphireError::ParseError {
                    message: "expected '}'".into(),
                    line: self.peek().line,
                });
            }
            self.advance(); // consume '}'
            Ok(vec![Expr::Begin {
                body,
                rescue_var,
                rescue_body,
                else_body: Vec::new(),
            }])
        } else {
            if !self.check(&TokenKind::RightBrace) {
                return Err(SapphireError::ParseError {
                    message: "expected '}'".into(),
                    line: self.peek().line,
                });
            }
            self.advance(); // consume '}'
            Ok(body)
        }
    }

    fn block(&mut self) -> Result<Vec<Expr>, SapphireError> {
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
            if self.check(&TokenKind::RightBrace) || self.is_at_end() {
                break;
            }
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
        if self.check(&TokenKind::If) {
            return self.if_expr();
        }
        if self.check(&TokenKind::Begin) {
            return self.begin_expr();
        }
        if self.check(&TokenKind::While) {
            return self.while_statement();
        }
        let mut left = self.range()?;
        while self.check(&TokenKind::AmpAmp) || self.check(&TokenKind::PipePipe) {
            let op = self.advance().clone();
            let right = self.range()?;
            left = Expr::Binary {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    // range: bitwise_or ('..' bitwise_or)?
    fn range(&mut self) -> Result<Expr, SapphireError> {
        let left = self.bitwise_or()?;
        if self.check(&TokenKind::DotDot) {
            self.advance();
            let right = self.bitwise_or()?;
            return Ok(Expr::Range {
                from: Box::new(left),
                to: Box::new(right),
            });
        }
        Ok(left)
    }

    // bitwise_or: bitwise_xor ('|' bitwise_xor)*
    fn bitwise_or(&mut self) -> Result<Expr, SapphireError> {
        let mut left = self.bitwise_xor()?;
        while self.check(&TokenKind::Pipe) {
            let op = self.advance().clone();
            let right = self.bitwise_xor()?;
            left = Expr::Binary {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    // bitwise_xor: bitwise_and ('^' bitwise_and)*
    fn bitwise_xor(&mut self) -> Result<Expr, SapphireError> {
        let mut left = self.bitwise_and()?;
        while self.check(&TokenKind::Caret) {
            let op = self.advance().clone();
            let right = self.bitwise_and()?;
            left = Expr::Binary {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    // bitwise_and: equality ('&' equality)*
    fn bitwise_and(&mut self) -> Result<Expr, SapphireError> {
        let mut left = self.equality()?;
        while self.check(&TokenKind::Amp) {
            let op = self.advance().clone();
            let right = self.equality()?;
            left = Expr::Binary {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    // equality: comparison (('==' | '!=') comparison)*
    fn equality(&mut self) -> Result<Expr, SapphireError> {
        let mut left = self.comparison()?;
        while self.check(&TokenKind::EqEq) || self.check(&TokenKind::BangEq) {
            let op = self.advance().clone();
            let right = self.comparison()?;
            left = Expr::Binary {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    // comparison: shift (('<' | '<=' | '>' | '>=') shift)*
    fn comparison(&mut self) -> Result<Expr, SapphireError> {
        let mut left = self.shift()?;
        while self.check(&TokenKind::Less)
            || self.check(&TokenKind::LessEq)
            || self.check(&TokenKind::Greater)
            || self.check(&TokenKind::GreaterEq)
        {
            let op = self.advance().clone();
            let right = self.shift()?;
            left = Expr::Binary {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    // shift: term (('<<' | '>>') term)*
    fn shift(&mut self) -> Result<Expr, SapphireError> {
        let mut left = self.term()?;
        while self.check(&TokenKind::LessLess) || self.check(&TokenKind::GreaterGreater) {
            let op = self.advance().clone();
            let right = self.term()?;
            left = Expr::Binary {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
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
        while self.check(&TokenKind::Star)
            || self.check(&TokenKind::Slash)
            || self.check(&TokenKind::Percent)
        {
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

    // unary: ('!' | '-' | '~') unary | call
    fn unary(&mut self) -> Result<Expr, SapphireError> {
        if self.check(&TokenKind::Bang)
            || self.check(&TokenKind::Minus)
            || self.check(&TokenKind::Tilde)
        {
            let op = self.advance().clone();
            let right = self.unary()?;
            return Ok(Expr::Unary {
                op,
                right: Box::new(right),
            });
        }
        self.call()
    }

    // call: primary ('(' args ')' | '.' IDENTIFIER)*
    fn call(&mut self) -> Result<Expr, SapphireError> {
        let mut expr = self.primary()?;
        loop {
            // Allow method chaining across newlines: `.map { |n| n * 2 }\n  .each { ... }`
            if self.check(&TokenKind::Newline) && self.next_non_newline_is_dot() {
                self.skip_terminators();
            }
            if self.check(&TokenKind::LeftParen) {
                expr = self.finish_call(expr)?;
            } else if self.check(&TokenKind::Dot) {
                self.advance(); // consume '.'
                let name = match self.peek().kind.clone() {
                    TokenKind::Identifier(n) => {
                        self.advance();
                        n
                    }
                    // Allow keywords as method/field names after '.' (e.g. self.class)
                    TokenKind::Class => {
                        self.advance();
                        "class".to_string()
                    }
                    _ => {
                        return Err(SapphireError::ParseError {
                            message: "expected field or method name after '.'".into(),
                            line: self.peek().line,
                        });
                    }
                };
                if self.check(&TokenKind::Eq) {
                    self.advance(); // consume '='
                    let value = self.logical()?;
                    expr = Expr::Set {
                        object: Box::new(expr),
                        name,
                        value: Box::new(value),
                    };
                    break;
                }
                if self.allow_trailing_block && self.is_block_start() {
                    let block = self.parse_block()?;
                    let get = Expr::Get {
                        object: Box::new(expr),
                        name,
                    };
                    expr = Expr::Call {
                        callee: Box::new(get),
                        args: Vec::new(),
                        block,
                    };
                    continue;
                }
                let get = Expr::Get {
                    object: Box::new(expr),
                    name,
                };
                if self.check(&TokenKind::LeftParen) {
                    expr = self.finish_call(get)?;
                } else {
                    expr = Expr::Call {
                        callee: Box::new(get),
                        args: Vec::new(),
                        block: None,
                    };
                }
            } else if self.check(&TokenKind::AmpDot) {
                self.advance(); // consume '&.'
                let name = match self.peek().kind.clone() {
                    TokenKind::Identifier(n) => {
                        self.advance();
                        n
                    }
                    _ => {
                        return Err(SapphireError::ParseError {
                            message: "expected method or field name after '&.'".into(),
                            line: self.peek().line,
                        });
                    }
                };
                if self.check(&TokenKind::LeftParen) {
                    let safe_get = Expr::SafeGet {
                        object: Box::new(expr),
                        name,
                    };
                    let call = self.finish_call(safe_get)?;
                    expr = call;
                } else {
                    expr = Expr::SafeGet {
                        object: Box::new(expr),
                        name,
                    };
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
                    expr = Expr::IndexSet {
                        object: Box::new(expr),
                        index: Box::new(index),
                        value: Box::new(value),
                    };
                    break;
                }
                expr = Expr::Index {
                    object: Box::new(expr),
                    index: Box::new(index),
                };
            } else {
                break;
            }
        }
        // bare identifier followed by a block: `each { |x| ... }` → implicit-self call
        if self.allow_trailing_block
            && let Expr::Variable(_) = &expr
            && self.is_block_start()
        {
            let block = self.parse_block()?;
            expr = Expr::Call {
                callee: Box::new(expr),
                args: Vec::new(),
                block,
            };
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
        Ok(Expr::Call {
            callee: Box::new(callee),
            args,
            block,
        })
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
                        TokenKind::Identifier(n) => {
                            self.advance();
                            params.push(n);
                        }
                        _ => {
                            return Err(SapphireError::ParseError {
                                message: "expected parameter name in block".into(),
                                line: self.peek().line,
                            });
                        }
                    }
                    if !self.check(&TokenKind::Comma) {
                        break;
                    }
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
            if self.check(&TokenKind::RightBrace) || self.is_at_end() {
                break;
            }
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
        if let TokenKind::Identifier(name) = self.peek().kind.clone()
            && self.current + 1 < self.tokens.len()
            && self.tokens[self.current + 1].kind == TokenKind::Colon
        {
            self.advance(); // consume identifier
            self.advance(); // consume ':'
            return Ok(CallArg {
                name: Some(name),
                value: self.logical()?,
            });
        }

        Ok(CallArg {
            name: None,
            value: self.logical()?,
        })
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

        if self.check(&TokenKind::Def) {
            return self.function_def();
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
            return Ok(Expr::Super {
                method,
                args,
                block,
            });
        }

        if let TokenKind::Identifier(name) = self.peek().kind.clone() {
            self.advance();
            if self.check(&TokenKind::Eq) {
                self.advance(); // consume '='
                let value = self.logical()?;
                return Ok(Expr::Assign {
                    name,
                    value: Box::new(value),
                });
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
                        TokenKind::Identifier(k) => {
                            self.advance();
                            k
                        }
                        _ => {
                            return Err(SapphireError::ParseError {
                                message: "expected key name in map literal".into(),
                                line: self.peek().line,
                            });
                        }
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
                    if !self.check(&TokenKind::Comma) {
                        break;
                    }
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
    use crate::ast::Expr;
    use crate::lexer::Lexer;

    fn parse_expr(source: &str) -> Expr {
        let tokens = Lexer::new(source).scan_tokens();
        let mut exprs = Parser::new(tokens).parse().unwrap();
        exprs.remove(0)
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
        let mut exprs = Parser::new(tokens).parse().unwrap();
        match exprs.remove(0) {
            Expr::Print(inner) => {
                assert!(matches!(*inner, Expr::Literal(Value::Int(42))));
            }
            other => panic!("expected print expr, got {:?}", other),
        }
    }

    #[test]
    fn test_multiple_statements() {
        let tokens = Lexer::new("x = 1; x + 2").scan_tokens();
        let stmts = Parser::new(tokens).parse().unwrap();
        assert_eq!(stmts.len(), 2);
    }

    #[test]
    fn test_class_def() {
        let tokens = Lexer::new("class Point { attr x; attr y }").scan_tokens();
        let mut exprs = Parser::new(tokens).parse().unwrap();
        assert!(matches!(
            exprs.remove(0),
            Expr::Class { name, .. } if name == "Point"
        ));
    }

    #[test]
    fn test_field_access() {
        // Without parens, dot access is now a zero-arg call (parens are optional)
        let expr = parse_expr("p.x");
        assert!(matches!(
            expr,
            Expr::Call { callee, args, .. }
            if args.is_empty() && matches!(callee.as_ref(), Expr::Get { name, .. } if name == "x")
        ));
    }

    #[test]
    fn test_named_arg_call() {
        let expr = parse_expr("Point.new(x: 1, y: 2)");
        assert!(matches!(expr, Expr::Call { .. }));
    }
}
