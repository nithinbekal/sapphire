use crate::ast::Expr;
use crate::environment::Environment;
use crate::error::SapphireError;
use crate::token::TokenKind;

pub fn evaluate(expr: Expr, env: &mut Environment) -> Result<i64, SapphireError> {
    match expr {
        Expr::Literal(n) => Ok(n),
        Expr::Grouping(inner) => evaluate(*inner, env),
        Expr::Variable(name) => env.get(&name).ok_or_else(|| SapphireError::RuntimeError {
            message: format!("undefined variable '{}'", name),
        }),
        Expr::Assign { name, value } => {
            let result = evaluate(*value, env)?;
            env.set(name, result);
            Ok(result)
        }
        Expr::Binary { left, op, right } => {
            let l = evaluate(*left, env)?;
            let r = evaluate(*right, env)?;
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
        evaluate(expr, &mut Environment::new()).unwrap()
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
        assert!(evaluate(expr, &mut Environment::new()).is_err());
    }

    #[test]
    fn test_assign_and_read() {
        let mut env = Environment::new();
        let tokens = Lexer::new("x = 10").scan_tokens();
        let expr = Parser::new(tokens).parse().unwrap();
        assert_eq!(evaluate(expr, &mut env).unwrap(), 10);

        let tokens = Lexer::new("x").scan_tokens();
        let expr = Parser::new(tokens).parse().unwrap();
        assert_eq!(evaluate(expr, &mut env).unwrap(), 10);
    }

    #[test]
    fn test_undefined_variable() {
        let tokens = Lexer::new("y").scan_tokens();
        let expr = Parser::new(tokens).parse().unwrap();
        assert!(evaluate(expr, &mut Environment::new()).is_err());
    }
}
