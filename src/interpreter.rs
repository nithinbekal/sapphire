use crate::ast::{Expr, Stmt};
use crate::environment::{EnvRef, Environment};
use crate::error::SapphireError;
use crate::token::TokenKind;
use crate::value::Value;

pub fn execute(stmt: Stmt, env: EnvRef) -> Result<Option<Value>, SapphireError> {
    match stmt {
        Stmt::Print(expr) => {
            let value = evaluate(expr, env)?;
            println!("{}", value);
            Ok(None)
        }
        Stmt::Expression(expr) => Ok(Some(evaluate(expr, env)?)),
        Stmt::Function { name, params, body } => {
            let func = Value::Function { params, body, closure: env.clone() };
            env.borrow_mut().set(name, func);
            Ok(None)
        }
        Stmt::While { condition, body } => {
            loop {
                let cond = evaluate(condition.clone(), env.clone())?;
                match cond {
                    Value::Bool(true) => {
                        for stmt in body.clone() {
                            execute(stmt, env.clone())?;
                        }
                    }
                    Value::Bool(false) => break,
                    _ => return Err(SapphireError::RuntimeError {
                        message: "while condition must be a boolean".into(),
                    }),
                }
            }
            Ok(None)
        }
        Stmt::If { condition, then_branch, else_branch } => {
            let cond = evaluate(condition, env.clone())?;
            let branch = match cond {
                Value::Bool(true)  => Some(then_branch),
                Value::Bool(false) => else_branch,
                _ => return Err(SapphireError::RuntimeError {
                    message: "if condition must be a boolean".into(),
                }),
            };
            if let Some(stmts) = branch {
                for stmt in stmts {
                    execute(stmt, env.clone())?;
                }
            }
            Ok(None)
        }
    }
}

pub fn evaluate(expr: Expr, env: EnvRef) -> Result<Value, SapphireError> {
    match expr {
        Expr::Literal(v) => Ok(v),
        Expr::Grouping(inner) => evaluate(*inner, env),
        Expr::Variable(name) => env.borrow().get(&name).ok_or_else(|| SapphireError::RuntimeError {
            message: format!("undefined variable '{}'", name),
        }),
        Expr::Assign { name, value } => {
            let result = evaluate(*value, env.clone())?;
            env.borrow_mut().set(name, result.clone());
            Ok(result)
        }
        Expr::Call { callee, args } => {
            let callee_val = evaluate(*callee, env.clone())?;
            let mut arg_vals = Vec::new();
            for arg in args {
                arg_vals.push(evaluate(arg, env.clone())?);
            }
            match callee_val {
                Value::Function { params, body, closure } => {
                    if params.len() != arg_vals.len() {
                        return Err(SapphireError::RuntimeError {
                            message: format!("expected {} argument(s), got {}", params.len(), arg_vals.len()),
                        });
                    }
                    let call_env = Environment::new_child(closure);
                    for (param, val) in params.iter().zip(arg_vals) {
                        call_env.borrow_mut().set(param.clone(), val);
                    }
                    let mut result = Value::Nil;
                    for stmt in body {
                        if let Some(v) = execute(stmt, call_env.clone())? {
                            result = v;
                        }
                    }
                    Ok(result)
                }
                _ => Err(SapphireError::RuntimeError {
                    message: "can only call functions".into(),
                }),
            }
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
            let l = evaluate(*left, env.clone())?;
            let r = evaluate(*right, env)?;
            match op.kind {
                TokenKind::EqEq   => Ok(Value::Bool(l == r)),
                TokenKind::BangEq => Ok(Value::Bool(l != r)),
                TokenKind::Plus => match (l, r) {
                    (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a + b)),
                    (Value::Str(a), Value::Str(b)) => Ok(Value::Str(a + &b)),
                    _ => Err(SapphireError::RuntimeError {
                        message: "'+' requires two integers or two strings".into(),
                    }),
                },
                _ => {
                    let (l, r) = match (l, r) {
                        (Value::Int(a), Value::Int(b)) => (a, b),
                        _ => return Err(SapphireError::RuntimeError {
                            message: format!("operator {:?} requires integers", op.kind),
                        }),
                    };
                    match op.kind {
                        TokenKind::Minus     => Ok(Value::Int(l - r)),
                        TokenKind::Star      => Ok(Value::Int(l * r)),
                        TokenKind::Slash     => {
                            if r == 0 {
                                Err(SapphireError::RuntimeError {
                                    message: "division by zero".into(),
                                })
                            } else {
                                Ok(Value::Int(l / r))
                            }
                        }
                        TokenKind::Less      => Ok(Value::Bool(l < r)),
                        TokenKind::LessEq    => Ok(Value::Bool(l <= r)),
                        TokenKind::Greater   => Ok(Value::Bool(l > r)),
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
        execute(stmts.remove(0), Environment::new()).unwrap().unwrap()
    }

    fn run_env(source: &str, env: EnvRef) -> Value {
        let tokens = Lexer::new(source).scan_tokens();
        let mut stmts = Parser::new(tokens).parse().unwrap();
        execute(stmts.remove(0), env).unwrap().unwrap()
    }

    fn exec_env(source: &str, env: EnvRef) {
        let tokens = Lexer::new(source).scan_tokens();
        let stmts = Parser::new(tokens).parse().unwrap();
        for stmt in stmts {
            execute(stmt, env.clone()).unwrap();
        }
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
        assert!(execute(stmts.remove(0), Environment::new()).is_err());
    }

    #[test]
    fn test_assign_and_read() {
        let env = Environment::new();
        assert_eq!(run_env("x = 10", env.clone()), Value::Int(10));
        assert_eq!(run_env("x", env.clone()), Value::Int(10));
    }

    #[test]
    fn test_undefined_variable() {
        let tokens = Lexer::new("y").scan_tokens();
        let mut stmts = Parser::new(tokens).parse().unwrap();
        assert!(execute(stmts.remove(0), Environment::new()).is_err());
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
    fn test_string_literal() {
        assert_eq!(run(r#""hello""#), Value::Str("hello".into()));
    }

    #[test]
    fn test_string_concat() {
        assert_eq!(run(r#""hello" + " world""#), Value::Str("hello world".into()));
    }

    #[test]
    fn test_string_equality() {
        assert_eq!(run(r#""a" == "a""#), Value::Bool(true));
        assert_eq!(run(r#""a" == "b""#), Value::Bool(false));
    }

    #[test]
    fn test_while() {
        let env = Environment::new();
        exec_env("x = 0; while x < 3 { x = x + 1 }", env.clone());
        assert_eq!(env.borrow().get("x"), Some(Value::Int(3)));
    }

    #[test]
    fn test_if_then() {
        let env = Environment::new();
        exec_env("x = 0; if true { x = 1 }", env.clone());
        assert_eq!(env.borrow().get("x"), Some(Value::Int(1)));
    }

    #[test]
    fn test_if_else() {
        let env = Environment::new();
        exec_env("if false { x = 1 } else { x = 2 }", env.clone());
        assert_eq!(env.borrow().get("x"), Some(Value::Int(2)));
    }

    #[test]
    fn test_if_condition() {
        let env = Environment::new();
        exec_env("x = 5; if x > 3 { x = 99 }", env.clone());
        assert_eq!(env.borrow().get("x"), Some(Value::Int(99)));
    }

    #[test]
    fn test_function_def_and_call() {
        let env = Environment::new();
        exec_env("def add(a, b) { a + b }", env.clone());
        let result = run_env("add(1, 2)", env);
        assert_eq!(result, Value::Int(3));
    }

    #[test]
    fn test_function_no_args() {
        let env = Environment::new();
        exec_env("def answer() { 42 }", env.clone());
        assert_eq!(run_env("answer()", env), Value::Int(42));
    }

    #[test]
    fn test_function_closure() {
        let env = Environment::new();
        exec_env("x = 10; def get_x() { x }", env.clone());
        assert_eq!(run_env("get_x()", env), Value::Int(10));
    }
}
