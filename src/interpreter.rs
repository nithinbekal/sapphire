use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;
use crate::ast::{Expr, Stmt};
use crate::environment::Environment;
use crate::value::EnvRef;
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
        Stmt::Class { name, fields, methods } => {
            let class = Value::Class { name: name.clone(), fields, methods, closure: env.clone() };
            env.borrow_mut().set(name, class);
            Ok(None)
        }
        Stmt::Function { name, params, body } => {
            let func = Value::Function { params, body, closure: env.clone() };
            env.borrow_mut().set(name, func);
            Ok(None)
        }
        Stmt::Return(expr) => {
            let value = evaluate(expr, env)?;
            Err(SapphireError::Return(value))
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
        Expr::SelfExpr => env.borrow().get("self").ok_or_else(|| SapphireError::RuntimeError {
            message: "self used outside of a method".into(),
        }),
        Expr::Get { object, name } => {
            let obj = evaluate(*object, env.clone())?;
            match obj {
                Value::Class { name: class_name, fields, methods, closure } => {
                    if name == "new" {
                        Ok(Value::Constructor { class_name, fields, methods, closure })
                    } else {
                        Err(SapphireError::RuntimeError {
                            message: format!("unknown class method '{}'", name),
                        })
                    }
                }
                Value::Instance { ref class_name, ref fields } => {
                    // Check fields first
                    if let Some(v) = fields.borrow().get(&name).cloned() {
                        return Ok(v);
                    }
                    // Look up method on the class
                    let class_val = env.borrow().get(class_name).ok_or_else(|| SapphireError::RuntimeError {
                        message: format!("class '{}' not found", class_name),
                    })?;
                    if let Value::Class { methods, closure, .. } = class_val {
                        if let Some(method) = methods.iter().find(|m| m.name == name) {
                            return Ok(Value::BoundMethod {
                                receiver: Box::new(obj),
                                params: method.params.clone(),
                                body: method.body.clone(),
                                closure,
                            });
                        }
                    }
                    Err(SapphireError::RuntimeError {
                        message: format!("undefined field or method '{}'", name),
                    })
                }
                _ => Err(SapphireError::RuntimeError {
                    message: format!("cannot access '{}' on this value", name),
                }),
            }
        }
        Expr::Set { object, name, value } => {
            let obj = evaluate(*object, env.clone())?;
            let val = evaluate(*value, env)?;
            match obj {
                Value::Instance { fields, .. } => {
                    fields.borrow_mut().insert(name, val.clone());
                    Ok(val)
                }
                _ => Err(SapphireError::RuntimeError {
                    message: "can only set fields on instances".into(),
                }),
            }
        }
        Expr::Call { callee, args } => {
            let callee_val = evaluate(*callee, env.clone())?;
            let mut eval_args: Vec<(Option<String>, Value)> = Vec::new();
            for arg in args {
                eval_args.push((arg.name, evaluate(arg.value, env.clone())?));
            }
            match callee_val {
                Value::Function { params, body, closure } => {
                    let arg_vals: Vec<Value> = eval_args.into_iter().map(|(_, v)| v).collect();
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
                        match execute(stmt, call_env.clone()) {
                            Ok(Some(v)) => result = v,
                            Ok(None) => {}
                            Err(SapphireError::Return(v)) => return Ok(v),
                            Err(e) => return Err(e),
                        }
                    }
                    Ok(result)
                }
                Value::BoundMethod { receiver, params, body, closure } => {
                    let arg_vals: Vec<Value> = eval_args.into_iter().map(|(_, v)| v).collect();
                    if params.len() != arg_vals.len() {
                        return Err(SapphireError::RuntimeError {
                            message: format!("expected {} argument(s), got {}", params.len(), arg_vals.len()),
                        });
                    }
                    let call_env = Environment::new_child(closure);
                    call_env.borrow_mut().set("self".to_string(), *receiver);
                    for (param, val) in params.iter().zip(arg_vals) {
                        call_env.borrow_mut().set(param.clone(), val);
                    }
                    let mut result = Value::Nil;
                    for stmt in body {
                        match execute(stmt, call_env.clone()) {
                            Ok(Some(v)) => result = v,
                            Ok(None) => {}
                            Err(SapphireError::Return(v)) => return Ok(v),
                            Err(e) => return Err(e),
                        }
                    }
                    Ok(result)
                }
                Value::Constructor { class_name, fields, .. } => {
                    let mut instance_fields: HashMap<String, Value> = HashMap::new();
                    // Apply declared defaults first
                    for field in &fields {
                        let val = match &field.default {
                            Some(expr) => evaluate(expr.clone(), env.clone())?,
                            None => Value::Nil,
                        };
                        instance_fields.insert(field.name.clone(), val);
                    }
                    // Apply named args
                    for (name_opt, val) in eval_args {
                        match name_opt {
                            Some(n) => {
                                if !instance_fields.contains_key(&n) {
                                    return Err(SapphireError::RuntimeError {
                                        message: format!("unknown field '{}'", n),
                                    });
                                }
                                instance_fields.insert(n, val);
                            }
                            None => return Err(SapphireError::RuntimeError {
                                message: "constructor requires named arguments (e.g. Point.new(x: 1, y: 2))".into(),
                            }),
                        }
                    }
                    Ok(Value::Instance { class_name, fields: Rc::new(RefCell::new(instance_fields)) })
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
    fn test_literal() { assert_eq!(run("42"), Value::Int(42)); }

    #[test]
    fn test_addition() { assert_eq!(run("1+2"), Value::Int(3)); }

    #[test]
    fn test_precedence() { assert_eq!(run("1+2*3"), Value::Int(7)); }

    #[test]
    fn test_grouping() { assert_eq!(run("(1+2)*3"), Value::Int(9)); }

    #[test]
    fn test_subtraction() { assert_eq!(run("10-3-2"), Value::Int(5)); }

    #[test]
    fn test_division() { assert_eq!(run("10/2"), Value::Int(5)); }

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
        assert_eq!(run_env("add(1, 2)", env), Value::Int(3));
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

    #[test]
    fn test_early_return() {
        let env = Environment::new();
        exec_env("def abs(n) { if n < 0 { return -n }; n }", env.clone());
        assert_eq!(run_env("abs(-5)", env.clone()), Value::Int(5));
        assert_eq!(run_env("abs(3)", env.clone()), Value::Int(3));
    }

    #[test]
    fn test_class_instantiation() {
        let env = Environment::new();
        exec_env("class Point { attr x: Int; attr y: Int }", env.clone());
        exec_env("p = Point.new(x: 3, y: 2)", env.clone());
        assert_eq!(run_env("p.x", env.clone()), Value::Int(3));
        assert_eq!(run_env("p.y", env.clone()), Value::Int(2));
    }

    #[test]
    fn test_instance_method() {
        let env = Environment::new();
        exec_env("class Point { attr x: Int; attr y: Int; def sum() { self.x + self.y } }", env.clone());
        exec_env("p = Point.new(x: 3, y: 2)", env.clone());
        assert_eq!(run_env("p.sum()", env.clone()), Value::Int(5));
    }

    #[test]
    fn test_method_with_arg() {
        let env = Environment::new();
        exec_env("class Point { attr x: Int; attr y: Int; def translate(dx) { self.x + dx } }", env.clone());
        exec_env("p = Point.new(x: 3, y: 2)", env.clone());
        assert_eq!(run_env("p.translate(10)", env.clone()), Value::Int(13));
    }

    #[test]
    fn test_field_mutation() {
        let env = Environment::new();
        exec_env("class Counter { attr n; def inc() { self.n = self.n + 1 } }", env.clone());
        exec_env("c = Counter.new(n: 0)", env.clone());
        exec_env("c.inc()", env.clone());
        assert_eq!(run_env("c.n", env.clone()), Value::Int(1));
    }

    #[test]
    fn test_class_default_field() {
        let env = Environment::new();
        exec_env(r#"class Point { attr x: Int; attr y: Int; attr label: Str = "origin" }"#, env.clone());
        exec_env("p = Point.new(x: 1, y: 2)", env.clone());
        assert_eq!(run_env("p.label", env.clone()), Value::Str("origin".into()));
    }
}
