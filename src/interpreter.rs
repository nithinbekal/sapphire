use crate::ast::{Expr, Stmt};
use crate::environment::Environment;
use crate::error::SapphireError;
use crate::token::TokenKind;
use crate::value::Value;

pub fn execute(stmt: Stmt, env: &mut Environment) -> Result<Option<Value>, SapphireError> {
    match stmt {
        Stmt::Print(expr) => {
            let value = evaluate(expr, env)?;
            println!("{}", value);
            Ok(None)
        }
        Stmt::Expression(expr) => Ok(Some(evaluate(expr, env)?)),
        Stmt::If { condition, then_branch, else_branch } => {
            let cond = evaluate(condition, env)?;
            let branch = match cond {
                Value::Bool(true)  => Some(then_branch),
                Value::Bool(false) => else_branch,
                _ => return Err(SapphireError::RuntimeError {
                    message: "if condition must be a boolean".into(),
                }),
            };
            if let Some(stmts) = branch {
                for stmt in stmts {
                    execute(stmt, env)?;
                }
            }
            Ok(None)
        }
    }
}

pub fn evaluate(expr: Expr, env: &mut Environment) -> Result<Value, SapphireError> {
    match expr {
        Expr::Literal(v) => Ok(v),
        Expr::Grouping(inner) => evaluate(*inner, env),
        Expr::Variable(name) => env.get(&name).ok_or_else(|| SapphireError::RuntimeError {
            message: format!("undefined variable '{}'", name),
        }),
        Expr::Assign { name, value } => {
            let result = evaluate(*value, env)?;
            env.set(name, result.clone());
            Ok(result)
        }
        Expr::Unary { op, right } => {
            let val = evaluate(*right, env)?;
            match op.kind {
                TokenKind::Bang => match val {
                    Value::Bool(b) => Ok(Value::Bool(!b)),
                    _ => Err(SapphireError::RuntimeError {
                        message: "expected boolean after '!'".into(),
                    }),
                },
                TokenKind::Minus => match val {
                    Value::Int(n) => Ok(Value::Int(-n)),
                    _ => Err(SapphireError::RuntimeError {
                        message: "expected integer after '-'".into(),
                    }),
                },
                _ => unreachable!(),
            }
        }
        Expr::Binary { left, op, right } => {
            let l = evaluate(*left, env)?;
            let r = evaluate(*right, env)?;
            match op.kind {
                TokenKind::EqEq  => Ok(Value::Bool(l == r)),
                TokenKind::BangEq => Ok(Value::Bool(l != r)),
                _ => {
                    let (l, r) = match (l, r) {
                        (Value::Int(a), Value::Int(b)) => (a, b),
                        _ => return Err(SapphireError::RuntimeError {
                            message: format!("operator {:?} requires integers", op.kind),
                        }),
                    };
                    match op.kind {
                        TokenKind::Plus  => Ok(Value::Int(l + r)),
                        TokenKind::Minus => Ok(Value::Int(l - r)),
                        TokenKind::Star  => Ok(Value::Int(l * r)),
                        TokenKind::Slash => {
                            if r == 0 {
                                Err(SapphireError::RuntimeError {
                                    message: "division by zero".into(),
                                })
                            } else {
                                Ok(Value::Int(l / r))
                            }
                        }
                        TokenKind::Less     => Ok(Value::Bool(l < r)),
                        TokenKind::LessEq   => Ok(Value::Bool(l <= r)),
                        TokenKind::Greater  => Ok(Value::Bool(l > r)),
                        TokenKind::GreaterEq => Ok(Value::Bool(l >= r)),
                        _ => Err(SapphireError::RuntimeError {
                            message: format!("unknown operator: {:?}", op.kind),
                        }),
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn run(source: &str) -> Value {
        let tokens = Lexer::new(source).scan_tokens();
        let mut stmts = Parser::new(tokens).parse().unwrap();
        execute(stmts.remove(0), &mut Environment::new()).unwrap().unwrap()
    }

    fn run_env<'a>(source: &str, env: &mut Environment) -> Value {
        let tokens = Lexer::new(source).scan_tokens();
        let mut stmts = Parser::new(tokens).parse().unwrap();
        execute(stmts.remove(0), env).unwrap().unwrap()
    }

    #[test]
    fn test_literal() {
        assert_eq!(run("42"), Value::Int(42));
    }

    #[test]
    fn test_addition() {
        assert_eq!(run("1+2"), Value::Int(3));
    }

    #[test]
    fn test_precedence() {
        assert_eq!(run("1+2*3"), Value::Int(7));
    }

    #[test]
    fn test_grouping() {
        assert_eq!(run("(1+2)*3"), Value::Int(9));
    }

    #[test]
    fn test_subtraction() {
        assert_eq!(run("10-3-2"), Value::Int(5));
    }

    #[test]
    fn test_division() {
        assert_eq!(run("10/2"), Value::Int(5));
    }

    #[test]
    fn test_division_by_zero() {
        let tokens = Lexer::new("1/0").scan_tokens();
        let mut stmts = Parser::new(tokens).parse().unwrap();
        assert!(execute(stmts.remove(0), &mut Environment::new()).is_err());
    }

    #[test]
    fn test_assign_and_read() {
        let mut env = Environment::new();
        assert_eq!(run_env("x = 10", &mut env), Value::Int(10));
        assert_eq!(run_env("x", &mut env), Value::Int(10));
    }

    #[test]
    fn test_undefined_variable() {
        let tokens = Lexer::new("y").scan_tokens();
        let mut stmts = Parser::new(tokens).parse().unwrap();
        assert!(execute(stmts.remove(0), &mut Environment::new()).is_err());
    }

    #[test]
    fn test_bool_literals() {
        assert_eq!(run("true"), Value::Bool(true));
        assert_eq!(run("false"), Value::Bool(false));
    }

    #[test]
    fn test_equality() {
        assert_eq!(run("1 == 1"), Value::Bool(true));
        assert_eq!(run("1 == 2"), Value::Bool(false));
        assert_eq!(run("true == true"), Value::Bool(true));
        assert_eq!(run("true == false"), Value::Bool(false));
    }

    #[test]
    fn test_comparison() {
        assert_eq!(run("1 < 2"), Value::Bool(true));
        assert_eq!(run("2 > 1"), Value::Bool(true));
        assert_eq!(run("1 <= 1"), Value::Bool(true));
        assert_eq!(run("1 >= 2"), Value::Bool(false));
    }

    #[test]
    fn test_bang() {
        assert_eq!(run("!true"), Value::Bool(false));
        assert_eq!(run("!false"), Value::Bool(true));
    }

    #[test]
    fn test_negate() {
        assert_eq!(run("-5"), Value::Int(-5));
        assert_eq!(run("-(1+2)"), Value::Int(-3));
    }

    #[test]
    fn test_if_then() {
        let mut env = Environment::new();
        run_env("x = 0", &mut env);
        let tokens = Lexer::new("if true { x = 1 }").scan_tokens();
        let mut stmts = Parser::new(tokens).parse().unwrap();
        execute(stmts.remove(0), &mut env).unwrap();
        assert_eq!(env.get("x"), Some(Value::Int(1)));
    }

    #[test]
    fn test_if_else() {
        let mut env = Environment::new();
        let tokens = Lexer::new("if false { x = 1 } else { x = 2 }").scan_tokens();
        let mut stmts = Parser::new(tokens).parse().unwrap();
        execute(stmts.remove(0), &mut env).unwrap();
        assert_eq!(env.get("x"), Some(Value::Int(2)));
    }

    #[test]
    fn test_if_condition() {
        let mut env = Environment::new();
        run_env("x = 5", &mut env);
        let tokens = Lexer::new("if x > 3 { x = 99 }").scan_tokens();
        let mut stmts = Parser::new(tokens).parse().unwrap();
        execute(stmts.remove(0), &mut env).unwrap();
        assert_eq!(env.get("x"), Some(Value::Int(99)));
    }
}
