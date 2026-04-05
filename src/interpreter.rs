use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;
use std::io::{self, Write};
use crate::ast::{Block, Expr, MethodDef, Stmt, StringPart};
use crate::environment::Environment;
use crate::value::EnvRef;
use crate::error::SapphireError;
use crate::token::TokenKind;
use crate::value::Value;

pub fn global_env() -> EnvRef {
    let env = Environment::new();
    env.borrow_mut().set("read_line".to_string(), Value::NativeFunction("read_line".to_string()));
    env
}

pub fn execute(stmt: Stmt, env: EnvRef) -> Result<Option<Value>, SapphireError> {
    match stmt {
        Stmt::Print(expr) => {
            let value = evaluate(expr, env)?;
            println!("{}", value);
            Ok(None)
        }
        Stmt::Expression(expr) => Ok(Some(evaluate(expr, env)?)),
        Stmt::Class { name, superclass, fields, methods } => {
            let (mut merged_fields, mut merged_methods) = match superclass {
                Some(ref super_name) => {
                    let super_val = env.borrow().get(super_name).ok_or_else(|| SapphireError::RuntimeError {
                        message: format!("superclass '{}' not found", super_name),
                    })?;
                    match super_val {
                        Value::Class { fields: sf, methods: sm, .. } => (sf, sm),
                        _ => return Err(SapphireError::RuntimeError {
                            message: format!("'{}' is not a class", super_name),
                        }),
                    }
                }
                None => (Vec::new(), Vec::new()),
            };
            merged_fields.extend(fields);
            for method in methods {
                merged_methods.retain(|m: &MethodDef| m.name != method.name);
                merged_methods.push(method);
            }
            let class = Value::Class { name: name.clone(), fields: merged_fields, methods: merged_methods, closure: env.clone() };
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
            if !env.borrow_mut().assign(&name, result.clone()) {
                env.borrow_mut().set(name, result.clone());
            }
            Ok(result)
        }
        Expr::SelfExpr => env.borrow().get("self").ok_or_else(|| SapphireError::RuntimeError {
            message: "self used outside of a method".into(),
        }),
        Expr::Get { object, name } => {
            let obj = evaluate(*object, env.clone())?;

            // Universal built-ins — checked first so they work on every type
            // (instances can override these by defining a method of the same name)
            if !matches!(obj, Value::Instance { .. }) {
                match name.as_str() {
                    "nil?" => return Ok(Value::Bool(obj == Value::Nil)),
                    "to_s" => return Ok(Value::Str(format!("{}", obj))),
                    "to_i" => return match obj {
                        Value::Int(n) => Ok(Value::Int(n)),
                        Value::Str(s) => s.trim().parse::<i64>().map(Value::Int).map_err(|_| SapphireError::RuntimeError {
                            message: format!("cannot convert {:?} to integer", s),
                        }),
                        _ => Err(SapphireError::RuntimeError {
                            message: format!("cannot convert {} to integer", obj),
                        }),
                    },
                    _ => {}
                }
            }

            // Instances: fields → class methods → built-in fallbacks
            if let Value::Instance { ref class_name, ref fields } = obj {
                if let Some(v) = fields.borrow().get(&name).cloned() {
                    return Ok(v);
                }
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
                // Built-in fallbacks for instances
                return match name.as_str() {
                    "nil?" => Ok(Value::Bool(false)),
                    "to_s" => Ok(Value::Str(format!("{}", obj))),
                    "to_i" => Err(SapphireError::RuntimeError {
                        message: format!("cannot convert {} to integer", obj),
                    }),
                    _ => Err(SapphireError::RuntimeError {
                        message: format!("undefined field or method '{}'", name),
                    }),
                };
            }

            // Arrays
            if let Value::List(ref elements) = obj {
                return match name.as_str() {
                    "length" => Ok(Value::Int(elements.borrow().len() as i64)),
                    "first" => elements.borrow().first().cloned().ok_or_else(|| SapphireError::RuntimeError {
                        message: "first called on empty list".into(),
                    }),
                    "last" => elements.borrow().last().cloned().ok_or_else(|| SapphireError::RuntimeError {
                        message: "last called on empty list".into(),
                    }),
                    "push" | "pop" | "each" | "map" | "select" | "reduce" => Ok(Value::NativeMethod {
                        receiver: Box::new(obj.clone()),
                        name,
                    }),
                    _ => Err(SapphireError::RuntimeError {
                        message: format!("unknown list method '{}'", name),
                    }),
                };
            }

            // Integers
            if let Value::Int(n) = obj {
                return match name.as_str() {
                    "times" => Ok(Value::NativeMethod {
                        receiver: Box::new(Value::Int(n)),
                        name,
                    }),
                    _ => Err(SapphireError::RuntimeError {
                        message: format!("unknown integer method '{}'", name),
                    }),
                };
            }

            // Strings
            if let Value::Str(ref s) = obj {
                return match name.as_str() {
                    "length" => Ok(Value::Int(s.chars().count() as i64)),
                    "upcase"          => Ok(Value::Str(s.to_uppercase())),
                    "downcase"        => Ok(Value::Str(s.to_lowercase())),
                    "strip"           => Ok(Value::Str(s.trim().to_string())),
                    "chomp"           => Ok(Value::Str(s.trim_end_matches('\n').trim_end_matches('\r').to_string())),
                    "empty?"          => Ok(Value::Bool(s.is_empty())),
                    "split" | "include?" | "starts_with?" | "ends_with?" => Ok(Value::NativeMethod {
                        receiver: Box::new(obj.clone()),
                        name,
                    }),
                    _ => Err(SapphireError::RuntimeError {
                        message: format!("unknown string method '{}'", name),
                    }),
                };
            }

            // Class: only .new
            if let Value::Class { name: class_name, fields, methods, closure } = obj {
                return if name == "new" {
                    Ok(Value::Constructor { class_name, fields, methods, closure })
                } else {
                    Err(SapphireError::RuntimeError {
                        message: format!("unknown class method '{}'", name),
                    })
                };
            }

            Err(SapphireError::RuntimeError {
                message: format!("cannot access '{}' on this value", obj),
            })
        }
        Expr::StringInterp(parts) => {
            let mut result = String::new();
            for part in parts {
                match part {
                    StringPart::Lit(s) => result.push_str(&s),
                    StringPart::Expr(expr) => {
                        let val = evaluate(*expr, env.clone())?;
                        result.push_str(&format!("{}", val));
                    }
                }
            }
            Ok(Value::Str(result))
        }
        Expr::ListLit(elements) => {
            let mut vals = Vec::new();
            for el in elements {
                vals.push(evaluate(el, env.clone())?);
            }
            Ok(Value::List(Rc::new(RefCell::new(vals))))
        }
        Expr::Index { object, index } => {
            let obj = evaluate(*object, env.clone())?;
            let idx = evaluate(*index, env)?;
            match (obj, idx) {
                (Value::List(elements), Value::Int(i)) => {
                    let elems = elements.borrow();
                    let i = if i < 0 { elems.len() as i64 + i } else { i };
                    elems.get(i as usize).cloned().ok_or_else(|| SapphireError::RuntimeError {
                        message: format!("index {} out of bounds", i),
                    })
                }
                _ => Err(SapphireError::RuntimeError {
                    message: "index operator requires a list and an integer".into(),
                }),
            }
        }
        Expr::IndexSet { object, index, value } => {
            let obj = evaluate(*object, env.clone())?;
            let idx = evaluate(*index, env.clone())?;
            let val = evaluate(*value, env)?;
            match (obj, idx) {
                (Value::List(elements), Value::Int(i)) => {
                    let mut elems = elements.borrow_mut();
                    let len = elems.len() as i64;
                    let i = if i < 0 { len + i } else { i };
                    if i < 0 || i >= len {
                        return Err(SapphireError::RuntimeError {
                            message: format!("index {} out of bounds", i),
                        });
                    }
                    elems[i as usize] = val.clone();
                    Ok(val)
                }
                _ => Err(SapphireError::RuntimeError {
                    message: "index assignment requires a list and an integer".into(),
                }),
            }
        }
        Expr::SafeGet { object, name } => {
            let obj = evaluate(*object, env.clone())?;
            if obj == Value::Nil {
                return Ok(Value::Nil);
            }
            evaluate(Expr::Get { object: Box::new(Expr::Literal(obj)), name }, env)
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
        Expr::Call { callee, args, block } => {
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
                Value::NativeFunction(name) => {
                    match name.as_str() {
                        "read_line" => {
                            io::stdout().flush().ok();
                            let mut line = String::new();
                            io::stdin().read_line(&mut line).map_err(|e| SapphireError::RuntimeError {
                                message: format!("read_line failed: {}", e),
                            })?;
                            Ok(Value::Str(line.trim_end_matches('\n').trim_end_matches('\r').to_string()))
                        }
                        _ => Err(SapphireError::RuntimeError {
                            message: format!("unknown native function '{}'", name),
                        }),
                    }
                }
                Value::NativeMethod { receiver, name } => {
                    let args: Vec<Value> = eval_args.into_iter().map(|(_, v)| v).collect();
                    match (*receiver, name.as_str()) {
                        (Value::List(elements), "each") => {
                            let blk = block.ok_or_else(|| SapphireError::RuntimeError {
                                message: "each requires a block".into(),
                            })?;
                            for val in elements.borrow().clone().iter() {
                                run_block(&blk, vec![val.clone()], env.clone())?;
                            }
                            Ok(Value::Nil)
                        }
                        (Value::List(elements), "map") => {
                            let blk = block.ok_or_else(|| SapphireError::RuntimeError {
                                message: "map requires a block".into(),
                            })?;
                            let mut result = Vec::new();
                            for val in elements.borrow().clone().iter() {
                                result.push(run_block(&blk, vec![val.clone()], env.clone())?);
                            }
                            Ok(Value::List(Rc::new(RefCell::new(result))))
                        }
                        (Value::List(elements), "select") => {
                            let blk = block.ok_or_else(|| SapphireError::RuntimeError {
                                message: "select requires a block".into(),
                            })?;
                            let mut result = Vec::new();
                            for val in elements.borrow().clone().iter() {
                                match run_block(&blk, vec![val.clone()], env.clone())? {
                                    Value::Bool(true) => result.push(val.clone()),
                                    Value::Bool(false) => {}
                                    _ => return Err(SapphireError::RuntimeError {
                                        message: "select block must return a boolean".into(),
                                    }),
                                }
                            }
                            Ok(Value::List(Rc::new(RefCell::new(result))))
                        }
                        (Value::List(elements), "reduce") => {
                            let blk = block.ok_or_else(|| SapphireError::RuntimeError {
                                message: "reduce requires a block".into(),
                            })?;
                            let elems = elements.borrow().clone();
                            let (mut acc, rest) = if args.len() == 1 {
                                (args.into_iter().next().unwrap(), elems.as_slice())
                            } else if args.is_empty() {
                                let mut it = elems.as_slice().split_first().ok_or_else(|| SapphireError::RuntimeError {
                                    message: "reduce requires an initial value or a non-empty list".into(),
                                })?;
                                (it.0.clone(), it.1)
                            } else {
                                return Err(SapphireError::RuntimeError {
                                    message: "reduce takes at most one argument".into(),
                                });
                            };
                            for val in rest {
                                acc = run_block(&blk, vec![acc, val.clone()], env.clone())?;
                            }
                            Ok(acc)
                        }
                        (Value::List(elements), "push") => {
                            if args.len() != 1 {
                                return Err(SapphireError::RuntimeError {
                                    message: "push requires exactly one argument".into(),
                                });
                            }
                            elements.borrow_mut().push(args.into_iter().next().unwrap());
                            Ok(Value::Nil)
                        }
                        (Value::List(elements), "pop") => {
                            elements.borrow_mut().pop().ok_or_else(|| SapphireError::RuntimeError {
                                message: "pop called on empty list".into(),
                            })
                        }
                        (Value::Int(n), "times") => {
                            let blk = block.ok_or_else(|| SapphireError::RuntimeError {
                                message: "times requires a block".into(),
                            })?;
                            for i in 0..n {
                                run_block(&blk, vec![Value::Int(i)], env.clone())?;
                            }
                            Ok(Value::Nil)
                        }
                        (Value::Str(s), "split") => {
                            let sep = match args.into_iter().next() {
                                Some(Value::Str(sep)) => sep,
                                _ => return Err(SapphireError::RuntimeError {
                                    message: "split requires a string argument".into(),
                                }),
                            };
                            let parts = s.split(sep.as_str())
                                .map(|p| Value::Str(p.to_string()))
                                .collect();
                            Ok(Value::List(Rc::new(RefCell::new(parts))))
                        }
                        (Value::Str(s), "include?") => {
                            let needle = match args.into_iter().next() {
                                Some(Value::Str(n)) => n,
                                _ => return Err(SapphireError::RuntimeError {
                                    message: "include? requires a string argument".into(),
                                }),
                            };
                            Ok(Value::Bool(s.contains(needle.as_str())))
                        }
                        (Value::Str(s), "starts_with?") => {
                            let prefix = match args.into_iter().next() {
                                Some(Value::Str(p)) => p,
                                _ => return Err(SapphireError::RuntimeError {
                                    message: "starts_with? requires a string argument".into(),
                                }),
                            };
                            Ok(Value::Bool(s.starts_with(prefix.as_str())))
                        }
                        (Value::Str(s), "ends_with?") => {
                            let suffix = match args.into_iter().next() {
                                Some(Value::Str(p)) => p,
                                _ => return Err(SapphireError::RuntimeError {
                                    message: "ends_with? requires a string argument".into(),
                                }),
                            };
                            Ok(Value::Bool(s.ends_with(suffix.as_str())))
                        }
                        _ => Err(SapphireError::RuntimeError {
                            message: "unknown native method".into(),
                        }),
                    }
                }
                // Allow calling a plain value with () as long as there are no args/block —
                // this makes x.to_s(), arr.length(), arr.first() etc. work identically to
                // their no-parens forms.
                v if eval_args.is_empty() && block.is_none() => Ok(v),
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
            // Short-circuit logical operators
            if op.kind == TokenKind::AmpAmp {
                let l = evaluate(*left, env.clone())?;
                return match l {
                    Value::Bool(false) => Ok(Value::Bool(false)),
                    Value::Bool(true) => evaluate(*right, env),
                    _ => Err(SapphireError::RuntimeError { message: "'&&' requires booleans".into() }),
                };
            }
            if op.kind == TokenKind::PipePipe {
                let l = evaluate(*left, env.clone())?;
                return match l {
                    Value::Bool(true) => Ok(Value::Bool(true)),
                    Value::Bool(false) => evaluate(*right, env),
                    _ => Err(SapphireError::RuntimeError { message: "'||' requires booleans".into() }),
                };
            }
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
                        TokenKind::Percent   => {
                            if r == 0 {
                                Err(SapphireError::RuntimeError {
                                    message: "division by zero".into(),
                                })
                            } else {
                                Ok(Value::Int(l % r))
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

fn run_block(block: &Block, args: Vec<Value>, env: EnvRef) -> Result<Value, SapphireError> {
    let block_env = Environment::new_child(env);
    for (param, val) in block.params.iter().zip(args) {
        block_env.borrow_mut().set(param.clone(), val);
    }
    let mut result = Value::Nil;
    for stmt in &block.body {
        match execute(stmt.clone(), block_env.clone()) {
            Ok(Some(v)) => result = v,
            Ok(None) => {}
            Err(SapphireError::Return(v)) => return Ok(v),
            Err(e) => return Err(e),
        }
    }
    Ok(result)
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
    fn test_modulo() {
        assert_eq!(run("10 % 3"), Value::Int(1));
        assert_eq!(run("9 % 3"), Value::Int(0));
    }

    #[test]
    fn test_times() {
        let env = Environment::new();
        exec_env("n = 0\n3.times { |i| n = n + 1 }", env.clone());
        assert_eq!(run_env("n", env.clone()), Value::Int(3));
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
    fn test_string_length() {
        assert_eq!(run(r#""hello".length"#), Value::Int(5));
        assert_eq!(run(r#""".empty?"#), Value::Bool(true));
        assert_eq!(run(r#""hi".empty?"#), Value::Bool(false));
    }

    #[test]
    fn test_string_case() {
        assert_eq!(run(r#""hello".upcase"#), Value::Str("HELLO".into()));
        assert_eq!(run(r#""HELLO".downcase"#), Value::Str("hello".into()));
    }

    #[test]
    fn test_string_strip() {
        assert_eq!(run(r#""  hi  ".strip"#), Value::Str("hi".into()));
        assert_eq!(run(r#""  hi  ".strip"#), Value::Str("hi".into()));
    }

    #[test]
    fn test_string_include() {
        assert_eq!(run(r#""hello".include?("ell")"#), Value::Bool(true));
        assert_eq!(run(r#""hello".include?("xyz")"#), Value::Bool(false));
    }

    #[test]
    fn test_string_starts_ends_with() {
        assert_eq!(run(r#""hello".starts_with?("hel")"#), Value::Bool(true));
        assert_eq!(run(r#""hello".ends_with?("llo")"#), Value::Bool(true));
        assert_eq!(run(r#""hello".starts_with?("xyz")"#), Value::Bool(false));
    }

    #[test]
    fn test_string_split() {
        let result = run(r#""a,b,c".split(",")"#);
        if let Value::List(parts) = result {
            let parts = parts.borrow();
            assert_eq!(parts[0], Value::Str("a".into()));
            assert_eq!(parts[1], Value::Str("b".into()));
            assert_eq!(parts[2], Value::Str("c".into()));
        } else {
            panic!("expected List");
        }
    }

    #[test]
    fn test_to_s() {
        assert_eq!(run("42.to_s"), Value::Str("42".into()));
        assert_eq!(run("true.to_s"), Value::Str("true".into()));
        assert_eq!(run("nil.to_s"), Value::Str("nil".into()));
    }

    #[test]
    fn test_to_i() {
        assert_eq!(run(r#""42".to_i"#), Value::Int(42));
        assert_eq!(run("42.to_i"), Value::Int(42));
    }

    #[test]
    fn test_safe_navigation_nil() {
        let env = Environment::new();
        exec_env("x = nil", env.clone());
        assert_eq!(run_env("x&.nil?", env.clone()), Value::Nil);
    }

    #[test]
    fn test_safe_navigation_non_nil() {
        let env = Environment::new();
        exec_env("class Point { attr x }; p = Point.new(x: 3)", env.clone());
        assert_eq!(run_env("p&.x", env.clone()), Value::Int(3));
    }

    #[test]
    fn test_nil_check() {
        assert_eq!(run("nil.nil?"), Value::Bool(true));
        assert_eq!(run("42.nil?"), Value::Bool(false));
        assert_eq!(run("\"hello\".nil?"), Value::Bool(false));
        assert_eq!(run("false.nil?"), Value::Bool(false));
    }

    #[test]
    fn test_each() {
        let env = Environment::new();
        exec_env("sum = 0; [1, 2, 3].each { |x| sum = sum + x }", env.clone());
        assert_eq!(env.borrow().get("sum"), Some(Value::Int(6)));
    }

    #[test]
    fn test_map() {
        let env = Environment::new();
        exec_env("result = [1, 2, 3].map { |x| x * 2 }", env.clone());
        assert_eq!(run_env("result[0]", env.clone()), Value::Int(2));
        assert_eq!(run_env("result[2]", env.clone()), Value::Int(6));
    }

    #[test]
    fn test_select() {
        let env = Environment::new();
        exec_env("result = [1, 2, 3, 4].select { |x| x > 2 }", env.clone());
        assert_eq!(run_env("result.length", env.clone()), Value::Int(2));
        assert_eq!(run_env("result[0]", env.clone()), Value::Int(3));
    }

    #[test]
    fn test_reduce_with_initial() {
        assert_eq!(run("[1, 2, 3, 4, 5].reduce(0) { |acc, n| acc + n }"), Value::Int(15));
    }

    #[test]
    fn test_reduce_without_initial() {
        assert_eq!(run("[1, 2, 3, 4, 5].reduce { |acc, n| acc * n }"), Value::Int(120));
    }

    #[test]
    fn test_string_interp() {
        let env = Environment::new();
        exec_env("name = \"world\"", env.clone());
        assert_eq!(run_env(r#""hello #{name}""#, env.clone()), Value::Str("hello world".into()));
    }

    #[test]
    fn test_string_interp_expr() {
        let env = Environment::new();
        exec_env("x = 3", env.clone());
        assert_eq!(run_env(r#""result: #{x * 2}""#, env.clone()), Value::Str("result: 6".into()));
    }

    #[test]
    fn test_string_interp_int() {
        let env = Environment::new();
        exec_env("n = 42", env.clone());
        assert_eq!(run_env(r#""n is #{n}""#, env.clone()), Value::Str("n is 42".into()));
    }

    #[test]
    fn test_list_literal() {
        let env = Environment::new();
        exec_env("a = [1, 2, 3]", env.clone());
        assert_eq!(run_env("a[0]", env.clone()), Value::Int(1));
        assert_eq!(run_env("a[2]", env.clone()), Value::Int(3));
    }

    #[test]
    fn test_list_index_set() {
        let env = Environment::new();
        exec_env("a = [1, 2, 3]", env.clone());
        exec_env("a[0] = 99", env.clone());
        assert_eq!(run_env("a[0]", env.clone()), Value::Int(99));
    }

    #[test]
    fn test_list_length() {
        let env = Environment::new();
        exec_env("a = [1, 2, 3]", env.clone());
        assert_eq!(run_env("a.length", env.clone()), Value::Int(3));
    }

    #[test]
    fn test_list_push() {
        let env = Environment::new();
        exec_env("a = [1, 2]", env.clone());
        exec_env("a.push(3)", env.clone());
        assert_eq!(run_env("a.length", env.clone()), Value::Int(3));
        assert_eq!(run_env("a[2]", env.clone()), Value::Int(3));
    }

    #[test]
    fn test_list_pop() {
        let env = Environment::new();
        exec_env("a = [1, 2, 3]", env.clone());
        assert_eq!(run_env("a.pop()", env.clone()), Value::Int(3));
        assert_eq!(run_env("a.length", env.clone()), Value::Int(2));
    }

    #[test]
    fn test_and() {
        assert_eq!(run("true && true"), Value::Bool(true));
        assert_eq!(run("true && false"), Value::Bool(false));
        assert_eq!(run("false && true"), Value::Bool(false));
    }

    #[test]
    fn test_or() {
        assert_eq!(run("true || false"), Value::Bool(true));
        assert_eq!(run("false || false"), Value::Bool(false));
        assert_eq!(run("false || true"), Value::Bool(true));
    }

    #[test]
    fn test_inheritance_fields() {
        let env = Environment::new();
        exec_env("class Animal { attr name }", env.clone());
        exec_env("class Dog < Animal { attr breed }", env.clone());
        exec_env("d = Dog.new(name: \"Rex\", breed: \"Lab\")", env.clone());
        assert_eq!(run_env("d.name", env.clone()), Value::Str("Rex".into()));
        assert_eq!(run_env("d.breed", env.clone()), Value::Str("Lab".into()));
    }

    #[test]
    fn test_inheritance_method() {
        let env = Environment::new();
        exec_env("class Animal { attr name; def speak() { \"...\" } }", env.clone());
        exec_env("class Dog < Animal {}", env.clone());
        exec_env("d = Dog.new(name: \"Rex\")", env.clone());
        assert_eq!(run_env("d.speak()", env.clone()), Value::Str("...".into()));
    }

    #[test]
    fn test_inheritance_override() {
        let env = Environment::new();
        exec_env("class Animal { attr name; def speak() { \"...\" } }", env.clone());
        exec_env("class Dog < Animal { def speak() { \"woof\" } }", env.clone());
        exec_env("d = Dog.new(name: \"Rex\")", env.clone());
        assert_eq!(run_env("d.speak()", env.clone()), Value::Str("woof".into()));
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
