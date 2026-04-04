use crate::ast::Expr;
use crate::error::SapphireError;
use crate::token::TokenKind;

pub fn evaluate(expr: Expr) -> Result<i64, SapphireError> {
    match expr {
        Expr::Literal(n) => Ok(n),
        Expr::Grouping(inner) => evaluate(*inner),
        Expr::Binary { left, op, right } => {
            let l = evaluate(*left)?;
            let r = evaluate(*right)?;
            match op.kind {
                TokenKind::Plus  => Ok(l + r),
                TokenKind::Minus => Ok(l - r),
                TokenKind::Star  => Ok(l * r),
                TokenKind::Slash => {
                    if r == 0 {
                        Err(SapphireError::RuntimeError {
                            message: "division by zero".into(),
                        })
                    } else {
                        Ok(l / r)
                    }
                }
                _ => Err(SapphireError::RuntimeError {
                    message: format!("unknown operator: {:?}", op.kind),
                }),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn run(source: &str) -> i64 {
        let tokens = Lexer::new(source).scan_tokens();
        let expr = Parser::new(tokens).parse().unwrap();
        evaluate(expr).unwrap()
    }

    #[test]
    fn test_literal() {
        assert_eq!(run("42"), 42);
    }

    #[test]
    fn test_addition() {
        assert_eq!(run("1+2"), 3);
    }

    #[test]
    fn test_precedence() {
        assert_eq!(run("1+2*3"), 7);
    }

    #[test]
    fn test_grouping() {
        assert_eq!(run("(1+2)*3"), 9);
    }

    #[test]
    fn test_subtraction() {
        assert_eq!(run("10-3-2"), 5);
    }

    #[test]
    fn test_division() {
        assert_eq!(run("10/2"), 5);
    }

    #[test]
    fn test_division_by_zero() {
        let tokens = Lexer::new("1/0").scan_tokens();
        let expr = Parser::new(tokens).parse().unwrap();
        assert!(evaluate(expr).is_err());
    }
}
