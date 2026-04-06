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

const LIST_STDLIB: &str = include_str!("../stdlib/list.spr");
const MAP_STDLIB: &str = include_str!("../stdlib/map.spr");

pub fn global_env() -> EnvRef {
    let env = Environment::new();
    env.borrow_mut().set("read_line".to_string(), Value::NativeFunction("read_line".to_string()));
    let object_class = Value::Class {
        name: "Object".to_string(),
        superclass: None,
        fields: Vec::new(),
        methods: Vec::new(),
        closure: env.clone(),
    };
    env.borrow_mut().set("Object".to_string(), object_class);
    env.borrow_mut().freeze("Object");
    for (src, label) in [(LIST_STDLIB, "stdlib/list.spr"), (MAP_STDLIB, "stdlib/map.spr")] {
        let tokens = crate::lexer::Lexer::new(src).scan_tokens();
        let stmts = crate::parser::Parser::new(tokens).parse()
            .unwrap_or_else(|e| panic!("{} failed to parse: {}", label, e));
        for stmt in stmts {
            execute(stmt, env.clone())
                .unwrap_or_else(|e| panic!("{} failed to execute: {}", label, e));
        }
    }
    env.borrow_mut().freeze("List");
    env.borrow_mut().freeze("Map");
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
            if env.borrow().is_frozen(&name) {
                return Err(SapphireError::RuntimeError {
                    message: format!("'{}' is reserved and cannot be redefined", name),
                });
            }
            let superclass: Option<String> = if superclass.is_none() && name != "Object" {
                Some("Object".to_string())
            } else {
                superclass
            };
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
            let class = Value::Class { name: name.clone(), superclass: superclass.clone(), fields: merged_fields, methods: merged_methods, closure: env.clone() };
            env.borrow_mut().set(name, class);
            Ok(None)
        }
        Stmt::Function { name, params, body } => {
            if env.borrow().is_frozen(&name) {
                return Err(SapphireError::RuntimeError {
                    message: format!("'{}' is reserved and cannot be redefined", name),
                });
            }
            let func = Value::Function { params, body, closure: env.clone() };
            env.borrow_mut().set(name, func);
            Ok(None)
        }
        Stmt::Return(expr) => {
            let value = evaluate(expr, env)?;
            Err(SapphireError::Return(value))
        }
        Stmt::Break(expr) => {
            let value = evaluate(expr, env)?;
            Err(SapphireError::Break(value))
        }
        Stmt::Next(expr) => {
            let value = evaluate(expr, env)?;
            Err(SapphireError::Next(value))
        }
        Stmt::MultiAssign { names, values } => {
            if names.len() != values.len() {
                return Err(SapphireError::RuntimeError {
                    message: format!("expected {} value(s), got {}", names.len(), values.len()),
                });
            }
            // Evaluate all RHS first so that `a, b = b, a` swaps correctly
            let vals: Vec<Value> = values.into_iter()
                .map(|e| evaluate(e, env.clone()))
                .collect::<Result<_, _>>()?;
            for (name, val) in names.into_iter().zip(vals) {
                if !env.borrow_mut().assign(&name, val.clone()) {
                    env.borrow_mut().set(name, val);
                }
            }
            Ok(None)
        }
        Stmt::Raise(expr) => {
            let value = evaluate(expr, env)?;
            Err(SapphireError::Raised(value))
        }
        Stmt::Begin { body, rescue_var, rescue_body, else_body } => {
            let mut result = Value::Nil;
            let mut caught: Option<Value> = None;
            for stmt in body {
                match execute(stmt, env.clone()) {
                    Ok(Some(v)) => result = v,
                    Ok(None) => {}
                    Err(SapphireError::Raised(v)) => { caught = Some(v); break; }
                    Err(SapphireError::RuntimeError { message }) => {
                        caught = Some(Value::Str(message));
                        break;
                    }
                    Err(e) => return Err(e),
                }
            }
            if let Some(err_val) = caught {
                if let Some(var) = rescue_var {
                    env.borrow_mut().set(var, err_val);
                }
                for stmt in rescue_body {
                    match execute(stmt, env.clone()) {
                        Ok(Some(v)) => result = v,
                        Ok(None) => {}
                        Err(e) => return Err(e),
                    }
                }
            } else {
                for stmt in else_body {
                    match execute(stmt, env.clone()) {
                        Ok(Some(v)) => result = v,
                        Ok(None) => {}
                        Err(e) => return Err(e),
                    }
                }
            }
            Ok(Some(result))
        }
        Stmt::While { condition, body } => {
            'while_loop: loop {
                let cond = evaluate(condition.clone(), env.clone())?;
                match cond {
                    Value::Bool(true) => {
                        for stmt in body.clone() {
                            match execute(stmt, env.clone()) {
                                Ok(_) => {}
                                Err(SapphireError::Break(_)) => break 'while_loop,
                                Err(SapphireError::Next(_)) => continue 'while_loop,
                                Err(e) => return Err(e),
                            }
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
        Expr::Variable(name) => {
            if let Some(v) = env.borrow().get(&name) {
                return Ok(v);
            }
            // Implicit self: if inside a method, fall back to self.name
            if let Some(self_val) = env.borrow().get("self") {
                if let Ok(v) = evaluate(
                    Expr::Get { object: Box::new(Expr::Literal(self_val)), name: name.clone() },
                    env.clone(),
                ) {
                    return Ok(v);
                }
            }
            Err(SapphireError::RuntimeError {
                message: format!("undefined variable '{}'", name),
            })
        }
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
                        Value::Float(f) => Ok(Value::Int(f as i64)),
                        Value::Str(s) => s.trim().parse::<i64>().map(Value::Int).map_err(|_| SapphireError::RuntimeError {
                            message: format!("cannot convert {:?} to integer", s),
                        }),
                        _ => Err(SapphireError::RuntimeError {
                            message: format!("cannot convert {} to integer", obj),
                        }),
                    },
                    "to_f" => return match obj {
                        Value::Float(f) => Ok(Value::Float(f)),
                        Value::Int(n) => Ok(Value::Float(n as f64)),
                        Value::Str(s) => s.trim().parse::<f64>().map(Value::Float).map_err(|_| SapphireError::RuntimeError {
                            message: format!("cannot convert {:?} to float", s),
                        }),
                        _ => Err(SapphireError::RuntimeError {
                            message: format!("cannot convert {} to float", obj),
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
                if let Value::Class { name: ref cname, methods, closure, .. } = class_val {
                    let cname = cname.clone();
                    if let Some(method) = methods.iter().find(|m| m.name == name) {
                        if method.private && env.borrow().get("__class__").is_none() {
                            return Err(SapphireError::RuntimeError {
                                message: format!("private method '{}' called from outside class", name),
                            });
                        }
                        return Ok(Value::BoundMethod {
                            receiver: Box::new(obj),
                            params: method.params.clone(),
                            body: method.body.clone(),
                            closure,
                            defined_in: cname,
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
                    "is_a?" => Ok(Value::NativeMethod {
                        receiver: Box::new(obj.clone()),
                        name,
                    }),
                    _ => Err(SapphireError::RuntimeError {
                        message: format!("undefined field or method '{}'", name),
                    }),
                };
            }

            // Arrays
            if let Value::List(ref elements) = obj {
                match name.as_str() {
                    "length" => return Ok(Value::Int(elements.borrow().len() as i64)),
                    "first" => return elements.borrow().first().cloned().ok_or_else(|| SapphireError::RuntimeError {
                        message: "first called on empty list".into(),
                    }),
                    "last" => return elements.borrow().last().cloned().ok_or_else(|| SapphireError::RuntimeError {
                        message: "last called on empty list".into(),
                    }),
                    "push" | "pop" | "each" | "reduce"
                    | "sort" | "sort_by" | "count" | "flatten" | "uniq" => return Ok(Value::NativeMethod {
                        receiver: Box::new(obj.clone()),
                        name,
                    }),
                    _ => {}
                }
                // Fall back to List class for stdlib-defined methods
                if let Some(Value::Class { name: class_name, methods, closure, .. }) = env.borrow().get("List") {
                    if let Some(method) = methods.iter().find(|m| m.name == name) {
                        return Ok(Value::BoundMethod {
                            receiver: Box::new(obj.clone()),
                            params: method.params.clone(),
                            body: method.body.clone(),
                            closure,
                            defined_in: class_name,
                        });
                    }
                }
                return Err(SapphireError::RuntimeError {
                    message: format!("unknown list method '{}'", name),
                });
            }

            // Ranges
            if let Value::Range { .. } = obj {
                match name.as_str() {
                    "each" | "include?" => return Ok(Value::NativeMethod {
                        receiver: Box::new(obj),
                        name,
                    }),
                    _ => return Err(SapphireError::RuntimeError {
                        message: format!("unknown range method '{}'", name),
                    }),
                }
            }

            // Maps
            if let Value::Map(ref map) = obj {
                match name.as_str() {
                    "length" => return Ok(Value::Int(map.borrow().len() as i64)),
                    "keys" => {
                        let mut keys: Vec<Value> = map.borrow().keys().map(|k| Value::Str(k.clone())).collect();
                        keys.sort_by(|a, b| a.to_string().cmp(&b.to_string()));
                        return Ok(Value::List(Rc::new(RefCell::new(keys))));
                    }
                    "values" => {
                        let mut pairs: Vec<(String, Value)> = map.borrow().iter()
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect();
                        pairs.sort_by(|a, b| a.0.cmp(&b.0));
                        return Ok(Value::List(Rc::new(RefCell::new(pairs.into_iter().map(|(_, v)| v).collect()))));
                    }
                    "has_key?" | "each" | "delete" | "merge" => return Ok(Value::NativeMethod {
                        receiver: Box::new(obj.clone()),
                        name,
                    }),
                    _ => {}
                }
                // Fall back to Map class for stdlib-defined methods
                if let Some(Value::Class { name: class_name, methods, closure, .. }) = env.borrow().get("Map") {
                    if let Some(method) = methods.iter().find(|m| m.name == name) {
                        return Ok(Value::BoundMethod {
                            receiver: Box::new(obj.clone()),
                            params: method.params.clone(),
                            body: method.body.clone(),
                            closure,
                            defined_in: class_name,
                        });
                    }
                }
                return Err(SapphireError::RuntimeError {
                    message: format!("unknown map method '{}'", name),
                });
            }

            // Integers
            if let Value::Int(n) = obj {
                return match name.as_str() {
                    "downto" => Ok(Value::NativeMethod {
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
                    "chars" => {
                        let chars: Vec<Value> = s.chars().map(|c| Value::Str(c.to_string())).collect();
                        Ok(Value::List(Rc::new(RefCell::new(chars))))
                    }
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
            if let Value::Class { name: class_name, fields, .. } = obj {
                return if name == "new" {
                    Ok(Value::Constructor { class_name, fields })
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
        Expr::MapLit(pairs) => {
            let mut map = HashMap::new();
            for (key, val_expr) in pairs {
                map.insert(key, evaluate(val_expr, env.clone())?);
            }
            Ok(Value::Map(Rc::new(RefCell::new(map))))
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
                (Value::Map(map), Value::Str(key)) => {
                    map.borrow().get(&key).cloned().ok_or_else(|| SapphireError::RuntimeError {
                        message: format!("key '{}' not found in map", key),
                    })
                }
                _ => Err(SapphireError::RuntimeError {
                    message: "index operator requires a list and integer, or map and string".into(),
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
                (Value::Map(map), Value::Str(key)) => {
                    map.borrow_mut().insert(key, val.clone());
                    Ok(val)
                }
                _ => Err(SapphireError::RuntimeError {
                    message: "index assignment requires a list and integer, or map and string".into(),
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
            // Intercept list.first(n) / list.last(n) before the callee is evaluated,
            // because Get("first") / Get("last") returns the element directly (not a callable).
            if let Expr::Get { object, name } = &*callee {
                if name == "first" || name == "last" {
                    let obj = evaluate(*object.clone(), env.clone())?;
                    if let Value::List(ref elements) = obj {
                        let arg_vals: Vec<Value> = args.iter()
                            .map(|a| evaluate(a.value.clone(), env.clone()))
                            .collect::<Result<_, _>>()?;
                        return match arg_vals.as_slice() {
                            [] => {
                                let elems = elements.borrow();
                                if name == "first" {
                                    elems.first().cloned().ok_or_else(|| SapphireError::RuntimeError {
                                        message: "first called on empty list".into(),
                                    })
                                } else {
                                    elems.last().cloned().ok_or_else(|| SapphireError::RuntimeError {
                                        message: "last called on empty list".into(),
                                    })
                                }
                            }
                            [Value::Int(n)] => {
                                let elems = elements.borrow();
                                let n = (*n).max(0) as usize;
                                let slice: Vec<Value> = if name == "first" {
                                    elems.iter().take(n).cloned().collect()
                                } else {
                                    let skip = elems.len().saturating_sub(n);
                                    elems.iter().skip(skip).cloned().collect()
                                };
                                Ok(Value::List(Rc::new(RefCell::new(slice))))
                            }
                            _ => Err(SapphireError::RuntimeError {
                                message: format!("{} takes 0 or 1 integer argument", name),
                            }),
                        };
                    }
                }
            }
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
                    if let Some(blk) = block {
                        call_env.borrow_mut().set("__block__".to_string(), Value::Block(blk, env.clone()));
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
                Value::BoundMethod { receiver, params, body, closure, defined_in } => {
                    let arg_vals: Vec<Value> = eval_args.into_iter().map(|(_, v)| v).collect();
                    if params.len() != arg_vals.len() {
                        return Err(SapphireError::RuntimeError {
                            message: format!("expected {} argument(s), got {}", params.len(), arg_vals.len()),
                        });
                    }
                    let call_env = Environment::new_child(closure);
                    call_env.borrow_mut().set("self".to_string(), *receiver);
                    call_env.borrow_mut().set("__class__".to_string(), Value::Str(defined_in));
                    for (param, val) in params.iter().zip(arg_vals) {
                        call_env.borrow_mut().set(param.clone(), val);
                    }
                    if let Some(blk) = block {
                        call_env.borrow_mut().set("__block__".to_string(), Value::Block(blk, env.clone()));
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
                                match run_block(&blk, vec![val.clone()], env.clone()) {
                                    Ok(_) => {}
                                    Err(SapphireError::Break(v)) => return Ok(v),
                                    Err(e) => return Err(e),
                                }
                            }
                            Ok(Value::Nil)
                        }
                        (Value::List(elements), "reduce") => {
                            let blk = block.ok_or_else(|| SapphireError::RuntimeError {
                                message: "reduce requires a block".into(),
                            })?;
                            let elems = elements.borrow().clone();
                            let (mut acc, rest) = if args.len() == 1 {
                                (args.into_iter().next().unwrap(), elems.as_slice())
                            } else if args.is_empty() {
                                let it = elems.as_slice().split_first().ok_or_else(|| SapphireError::RuntimeError {
                                    message: "reduce requires an initial value or a non-empty list".into(),
                                })?;
                                (it.0.clone(), it.1)
                            } else {
                                return Err(SapphireError::RuntimeError {
                                    message: "reduce takes at most one argument".into(),
                                });
                            };
                            for val in rest {
                                match run_block(&blk, vec![acc.clone(), val.clone()], env.clone()) {
                                    Ok(v) => acc = v,
                                    Err(SapphireError::Break(v)) => return Ok(v),
                                    Err(e) => return Err(e),
                                }
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
                        (Value::List(elements), "count") => {
                            if let Some(blk) = block {
                                let mut n = 0i64;
                                for val in elements.borrow().clone().iter() {
                                    match run_block(&blk, vec![val.clone()], env.clone()) {
                                        Ok(Value::Bool(true)) => n += 1,
                                        Ok(_) => {}
                                        Err(SapphireError::Break(v)) => return Ok(v),
                                        Err(e) => return Err(e),
                                    }
                                }
                                Ok(Value::Int(n))
                            } else {
                                Ok(Value::Int(elements.borrow().len() as i64))
                            }
                        }
                        (Value::List(elements), "sort") => {
                            let mut sorted = elements.borrow().clone();
                            sorted.sort_by(|a, b| match (a, b) {
                                (Value::Int(x),   Value::Int(y))   => x.cmp(y),
                                (Value::Float(x), Value::Float(y)) => x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal),
                                (Value::Int(x),   Value::Float(y)) => (*x as f64).partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal),
                                (Value::Float(x), Value::Int(y))   => x.partial_cmp(&(*y as f64)).unwrap_or(std::cmp::Ordering::Equal),
                                (Value::Str(x),   Value::Str(y))   => x.cmp(y),
                                _ => std::cmp::Ordering::Equal,
                            });
                            Ok(Value::List(Rc::new(RefCell::new(sorted))))
                        }
                        (Value::List(elements), "sort_by") => {
                            let blk = block.ok_or_else(|| SapphireError::RuntimeError {
                                message: "sort_by requires a block".into(),
                            })?;
                            let elems = elements.borrow().clone();
                            let mut keyed: Vec<(Value, Value)> = Vec::new();
                            for val in elems {
                                let key = run_block(&blk, vec![val.clone()], env.clone())?;
                                keyed.push((val, key));
                            }
                            keyed.sort_by(|(_, a), (_, b)| match (a, b) {
                                (Value::Int(x),   Value::Int(y))   => x.cmp(y),
                                (Value::Float(x), Value::Float(y)) => x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal),
                                (Value::Int(x),   Value::Float(y)) => (*x as f64).partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal),
                                (Value::Float(x), Value::Int(y))   => x.partial_cmp(&(*y as f64)).unwrap_or(std::cmp::Ordering::Equal),
                                (Value::Str(x),   Value::Str(y))   => x.cmp(y),
                                _ => std::cmp::Ordering::Equal,
                            });
                            Ok(Value::List(Rc::new(RefCell::new(keyed.into_iter().map(|(v, _)| v).collect()))))
                        }
                        (Value::List(elements), "flatten") => {
                            fn do_flatten(val: Value) -> Vec<Value> {
                                match val {
                                    Value::List(inner) => inner.borrow().clone().into_iter().flat_map(do_flatten).collect(),
                                    other => vec![other],
                                }
                            }
                            let result: Vec<Value> = elements.borrow().clone().into_iter().flat_map(do_flatten).collect();
                            Ok(Value::List(Rc::new(RefCell::new(result))))
                        }
                        (Value::List(elements), "uniq") => {
                            let mut seen: Vec<Value> = Vec::new();
                            for val in elements.borrow().clone() {
                                if !seen.contains(&val) {
                                    seen.push(val);
                                }
                            }
                            Ok(Value::List(Rc::new(RefCell::new(seen))))
                        }
                        (Value::Range { from, to }, "each") => {
                            let blk = block.ok_or_else(|| SapphireError::RuntimeError {
                                message: "each requires a block".into(),
                            })?;
                            let mut i = from;
                            while i <= to {
                                match run_block(&blk, vec![Value::Int(i)], env.clone()) {
                                    Ok(_) => {}
                                    Err(SapphireError::Break(v)) => return Ok(v),
                                    Err(e) => return Err(e),
                                }
                                i += 1;
                            }
                            Ok(Value::Nil)
                        }
                        (Value::Range { from, to }, "include?") => {
                            let n = match args.into_iter().next() {
                                Some(Value::Int(n)) => n,
                                _ => return Err(SapphireError::RuntimeError {
                                    message: "include? requires an integer argument".into(),
                                }),
                            };
                            Ok(Value::Bool(n >= from && n <= to))
                        }
                        (Value::Map(map), "has_key?") => {
                            let key = match args.into_iter().next() {
                                Some(Value::Str(k)) => k,
                                _ => return Err(SapphireError::RuntimeError {
                                    message: "has_key? requires a string argument".into(),
                                }),
                            };
                            Ok(Value::Bool(map.borrow().contains_key(&key)))
                        }
                        (Value::Map(map), "delete") => {
                            let key = match args.into_iter().next() {
                                Some(Value::Str(k)) => k,
                                _ => return Err(SapphireError::RuntimeError {
                                    message: "delete requires a string argument".into(),
                                }),
                            };
                            Ok(map.borrow_mut().remove(&key).unwrap_or(Value::Nil))
                        }
                        (Value::Map(map), "merge") => {
                            let other = match args.into_iter().next() {
                                Some(Value::Map(m)) => m,
                                _ => return Err(SapphireError::RuntimeError {
                                    message: "merge requires a map argument".into(),
                                }),
                            };
                            let mut result = map.borrow().clone();
                            for (k, v) in other.borrow().iter() {
                                result.insert(k.clone(), v.clone());
                            }
                            Ok(Value::Map(Rc::new(RefCell::new(result))))
                        }
                        (Value::Map(map), "each") => {
                            let blk = block.ok_or_else(|| SapphireError::RuntimeError {
                                message: "each requires a block".into(),
                            })?;
                            let mut pairs: Vec<(String, Value)> = map.borrow().iter()
                                .map(|(k, v)| (k.clone(), v.clone()))
                                .collect();
                            pairs.sort_by(|a, b| a.0.cmp(&b.0));
                            for (k, v) in pairs {
                                match run_block(&blk, vec![Value::Str(k), v], env.clone()) {
                                    Ok(_) => {}
                                    Err(SapphireError::Break(val)) => return Ok(val),
                                    Err(e) => return Err(e),
                                }
                            }
                            Ok(Value::Nil)
                        }
                        (Value::Int(from), "downto") => {
                            let to = match args.into_iter().next() {
                                Some(Value::Int(n)) => n,
                                _ => return Err(SapphireError::RuntimeError {
                                    message: "downto requires an integer argument".into(),
                                }),
                            };
                            let blk = block.ok_or_else(|| SapphireError::RuntimeError {
                                message: "downto requires a block".into(),
                            })?;
                            let mut i = from;
                            while i >= to {
                                match run_block(&blk, vec![Value::Int(i)], env.clone()) {
                                    Ok(_) => {}
                                    Err(SapphireError::Break(v)) => return Ok(v),
                                    Err(e) => return Err(e),
                                }
                                i -= 1;
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
                        (Value::Instance { class_name, .. }, "is_a?") => {
                            let target = match args.into_iter().next() {
                                Some(Value::Str(s)) => s,
                                _ => return Err(SapphireError::RuntimeError {
                                    message: "is_a? requires a string argument".into(),
                                }),
                            };
                            let mut current: Option<String> = Some(class_name);
                            loop {
                                match current {
                                    None => return Ok(Value::Bool(false)),
                                    Some(ref cname) => {
                                        if *cname == target { return Ok(Value::Bool(true)); }
                                        let cname_owned = cname.clone();
                                        match env.borrow().get(&cname_owned) {
                                            Some(Value::Class { superclass, .. }) => current = superclass,
                                            _ => return Ok(Value::Bool(false)),
                                        }
                                    }
                                }
                            }
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
                    Value::Float(f) => Ok(Value::Float(-f)),
                    _ => Err(SapphireError::RuntimeError {
                        message: "expected number after '-'".into(),
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
                    (Value::Int(a),   Value::Int(b))   => Ok(Value::Int(a + b)),
                    (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
                    (Value::Int(a),   Value::Float(b)) => Ok(Value::Float(a as f64 + b)),
                    (Value::Float(a), Value::Int(b))   => Ok(Value::Float(a + b as f64)),
                    (Value::Str(a),   Value::Str(b))   => Ok(Value::Str(a + &b)),
                    _ => Err(SapphireError::RuntimeError {
                        message: "'+' requires two numbers or two strings".into(),
                    }),
                },
                op_kind => {
                    // Promote to float if either operand is a float; otherwise require both ints.
                    let use_float = matches!((&l, &r), (Value::Float(_), _) | (_, Value::Float(_)));
                    if use_float {
                        let a = match l {
                            Value::Int(n)   => n as f64,
                            Value::Float(f) => f,
                            _ => return Err(SapphireError::RuntimeError {
                                message: format!("operator {:?} requires numbers", op_kind),
                            }),
                        };
                        let b = match r {
                            Value::Int(n)   => n as f64,
                            Value::Float(f) => f,
                            _ => return Err(SapphireError::RuntimeError {
                                message: format!("operator {:?} requires numbers", op_kind),
                            }),
                        };
                        match op_kind {
                            TokenKind::Minus     => Ok(Value::Float(a - b)),
                            TokenKind::Star      => Ok(Value::Float(a * b)),
                            TokenKind::Slash     => Ok(Value::Float(a / b)),
                            TokenKind::Percent   => Ok(Value::Float(a % b)),
                            TokenKind::Less      => Ok(Value::Bool(a < b)),
                            TokenKind::LessEq    => Ok(Value::Bool(a <= b)),
                            TokenKind::Greater   => Ok(Value::Bool(a > b)),
                            TokenKind::GreaterEq => Ok(Value::Bool(a >= b)),
                            _ => Err(SapphireError::RuntimeError {
                                message: format!("unknown operator: {:?}", op_kind),
                            }),
                        }
                    } else {
                        let (a, b) = match (l, r) {
                            (Value::Int(a), Value::Int(b)) => (a, b),
                            _ => return Err(SapphireError::RuntimeError {
                                message: format!("operator {:?} requires numbers", op_kind),
                            }),
                        };
                        match op_kind {
                            TokenKind::Minus     => Ok(Value::Int(a - b)),
                            TokenKind::Star      => Ok(Value::Int(a * b)),
                            TokenKind::Slash     => {
                                if b == 0 {
                                    Err(SapphireError::RuntimeError { message: "division by zero".into() })
                                } else {
                                    Ok(Value::Int(a / b))
                                }
                            }
                            TokenKind::Percent   => {
                                if b == 0 {
                                    Err(SapphireError::RuntimeError { message: "division by zero".into() })
                                } else {
                                    Ok(Value::Int(a % b))
                                }
                            }
                            TokenKind::Less      => Ok(Value::Bool(a < b)),
                            TokenKind::LessEq    => Ok(Value::Bool(a <= b)),
                            TokenKind::Greater   => Ok(Value::Bool(a > b)),
                            TokenKind::GreaterEq => Ok(Value::Bool(a >= b)),
                            _ => Err(SapphireError::RuntimeError {
                                message: format!("unknown operator: {:?}", op_kind),
                            }),
                        }
                    }
                }
            }
        }
        Expr::Yield { args } => {
            let block_val = env.borrow().get("__block__").ok_or_else(|| SapphireError::RuntimeError {
                message: "yield called outside of a method that received a block".into(),
            })?;
            let (blk, block_env) = match block_val {
                Value::Block(b, e) => (b, e),
                _ => return Err(SapphireError::RuntimeError {
                    message: "yield: __block__ is not a block".into(),
                }),
            };
            let mut arg_vals = Vec::new();
            for arg in args {
                arg_vals.push(evaluate(arg.value, env.clone())?);
            }
            run_block(&blk, arg_vals, block_env)
        }
        Expr::Range { from, to } => {
            let from_val = evaluate(*from, env.clone())?;
            let to_val = evaluate(*to, env)?;
            match (from_val, to_val) {
                (Value::Int(f), Value::Int(t)) => Ok(Value::Range { from: f, to: t }),
                _ => Err(SapphireError::RuntimeError {
                    message: "range bounds must be integers".into(),
                }),
            }
        }
        Expr::Super { method, args, block } => {
            let self_val = env.borrow().get("self").ok_or_else(|| SapphireError::RuntimeError {
                message: "super used outside of a method".into(),
            })?;
            let current_class_name = match env.borrow().get("__class__") {
                Some(Value::Str(s)) => s,
                _ => return Err(SapphireError::RuntimeError {
                    message: "super used outside of a method".into(),
                }),
            };
            let current_class = env.borrow().get(&current_class_name).ok_or_else(|| SapphireError::RuntimeError {
                message: format!("class '{}' not found", current_class_name),
            })?;
            let super_name = match current_class {
                Value::Class { superclass: Some(s), .. } => s,
                _ => return Err(SapphireError::RuntimeError {
                    message: format!("'{}' has no superclass", current_class_name),
                }),
            };
            let super_class = env.borrow().get(&super_name).ok_or_else(|| SapphireError::RuntimeError {
                message: format!("superclass '{}' not found", super_name),
            })?;
            let (super_methods, super_closure) = match super_class {
                Value::Class { methods, closure, .. } => (methods, closure),
                _ => return Err(SapphireError::RuntimeError {
                    message: format!("'{}' is not a class", super_name),
                }),
            };
            let meth = super_methods.iter().find(|m| m.name == method).ok_or_else(|| SapphireError::RuntimeError {
                message: format!("superclass '{}' has no method '{}'", super_name, method),
            })?;
            let mut arg_vals: Vec<Value> = Vec::new();
            for arg in args {
                arg_vals.push(evaluate(arg.value, env.clone())?);
            }
            if meth.params.len() != arg_vals.len() {
                return Err(SapphireError::RuntimeError {
                    message: format!("expected {} argument(s), got {}", meth.params.len(), arg_vals.len()),
                });
            }
            let call_env = Environment::new_child(super_closure);
            call_env.borrow_mut().set("self".to_string(), self_val);
            call_env.borrow_mut().set("__class__".to_string(), Value::Str(super_name));
            for (param, val) in meth.params.iter().zip(arg_vals) {
                call_env.borrow_mut().set(param.clone(), val);
            }
            let _ = block; // super calls don't support blocks for now
            let mut result = Value::Nil;
            for stmt in meth.body.clone() {
                match execute(stmt, call_env.clone()) {
                    Ok(Some(v)) => result = v,
                    Ok(None) => {}
                    Err(SapphireError::Return(v)) => return Ok(v),
                    Err(e) => return Err(e),
                }
            }
            Ok(result)
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
            Err(SapphireError::Next(v)) => return Ok(v),
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
        let env = global_env();
        exec_env("class Point { attr x; attr y }", env.clone());
        exec_env("p = Point.new(x: 3, y: 2)", env.clone());
        assert_eq!(run_env("p.x", env.clone()), Value::Int(3));
        assert_eq!(run_env("p.y", env.clone()), Value::Int(2));
    }

    #[test]
    fn test_instance_method() {
        let env = global_env();
        exec_env("class Point { attr x; attr y; def sum() { self.x + self.y } }", env.clone());
        exec_env("p = Point.new(x: 3, y: 2)", env.clone());
        assert_eq!(run_env("p.sum()", env.clone()), Value::Int(5));
    }

    #[test]
    fn test_method_with_arg() {
        let env = global_env();
        exec_env("class Point { attr x; attr y; def translate(dx) { self.x + dx } }", env.clone());
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
        let env = global_env();
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
        let env = global_env();
        exec_env("result = [1, 2, 3].map { |x| x * 2 }", env.clone());
        assert_eq!(run_env("result[0]", env.clone()), Value::Int(2));
        assert_eq!(run_env("result[2]", env.clone()), Value::Int(6));
    }

    #[test]
    fn test_select() {
        let env = global_env();
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
        let env = global_env();
        exec_env("class Animal { attr name }", env.clone());
        exec_env("class Dog < Animal { attr breed }", env.clone());
        exec_env("d = Dog.new(name: \"Rex\", breed: \"Lab\")", env.clone());
        assert_eq!(run_env("d.name", env.clone()), Value::Str("Rex".into()));
        assert_eq!(run_env("d.breed", env.clone()), Value::Str("Lab".into()));
    }

    #[test]
    fn test_inheritance_method() {
        let env = global_env();
        exec_env("class Animal { attr name; def speak() { \"...\" } }", env.clone());
        exec_env("class Dog < Animal {}", env.clone());
        exec_env("d = Dog.new(name: \"Rex\")", env.clone());
        assert_eq!(run_env("d.speak()", env.clone()), Value::Str("...".into()));
    }

    #[test]
    fn test_inheritance_override() {
        let env = global_env();
        exec_env("class Animal { attr name; def speak() { \"...\" } }", env.clone());
        exec_env("class Dog < Animal { def speak() { \"woof\" } }", env.clone());
        exec_env("d = Dog.new(name: \"Rex\")", env.clone());
        assert_eq!(run_env("d.speak()", env.clone()), Value::Str("woof".into()));
    }

    #[test]
    fn test_field_mutation() {
        let env = global_env();
        exec_env("class Counter { attr n; def inc() { self.n = self.n + 1 } }", env.clone());
        exec_env("c = Counter.new(n: 0)", env.clone());
        exec_env("c.inc()", env.clone());
        assert_eq!(run_env("c.n", env.clone()), Value::Int(1));
    }

    #[test]
    fn test_class_default_field() {
        let env = global_env();
        exec_env(r#"class Point { attr x; attr y; attr label = "origin" }"#, env.clone());
        exec_env("p = Point.new(x: 1, y: 2)", env.clone());
        assert_eq!(run_env("p.label", env.clone()), Value::Str("origin".into()));
    }

    #[test]
    fn test_while_break() {
        let env = Environment::new();
        exec_env("x = 0; while true { x = x + 1; break if x == 3 }", env.clone());
        assert_eq!(env.borrow().get("x"), Some(Value::Int(3)));
    }

    #[test]
    fn test_while_next() {
        let env = Environment::new();
        exec_env("x = 0; sum = 0; while x < 5 { x = x + 1; next if x == 3; sum = sum + x }", env.clone());
        assert_eq!(env.borrow().get("sum"), Some(Value::Int(12))); // 1+2+4+5
    }

    #[test]
    fn test_each_next() {
        let env = Environment::new();
        exec_env("sum = 0; [1, 2, 3, 4, 5].each { |x| next if x == 3; sum = sum + x }", env.clone());
        assert_eq!(env.borrow().get("sum"), Some(Value::Int(12))); // 1+2+4+5
    }

    #[test]
    fn test_each_break() {
        let env = Environment::new();
        exec_env("sum = 0; [1, 2, 3, 4, 5].each { |x| break if x == 4; sum = sum + x }", env.clone());
        assert_eq!(env.borrow().get("sum"), Some(Value::Int(6))); // 1+2+3
    }

    #[test]
    fn test_super() {
        let env = global_env();
        exec_env("class Animal { attr name; def speak() { \"...\" } }", env.clone());
        exec_env("class Dog < Animal { def speak() { super.speak() } }", env.clone());
        exec_env("d = Dog.new(name: \"Rex\")", env.clone());
        assert_eq!(run_env("d.speak()", env.clone()), Value::Str("...".into()));
    }

    #[test]
    fn test_super_with_override() {
        let env = global_env();
        exec_env("class Animal { attr name; def describe() { self.name } }", env.clone());
        exec_env("class Dog < Animal { attr breed; def describe() { super.describe() + \" (\" + self.breed + \")\" } }", env.clone());
        exec_env("d = Dog.new(name: \"Rex\", breed: \"Lab\")", env.clone());
        assert_eq!(run_env("d.describe()", env.clone()), Value::Str("Rex (Lab)".into()));
    }

    #[test]
    fn test_map_next() {
        let env = global_env();
        exec_env("result = [1, 2, 3].map { |x| next 0 if x == 2; x * 2 }", env.clone());
        assert_eq!(run_env("result[0]", env.clone()), Value::Int(2));
        assert_eq!(run_env("result[1]", env.clone()), Value::Int(0));
        assert_eq!(run_env("result[2]", env.clone()), Value::Int(6));
    }

    #[test]
    fn test_yield_basic() {
        let env = Environment::new();
        exec_env("def call_block() { yield(42) }", env.clone());
        assert_eq!(run_env("call_block() { |x| x * 2 }", env.clone()), Value::Int(84));
    }

    #[test]
    fn test_yield_multiple_args() {
        let env = Environment::new();
        exec_env("def call_block(a, b) { yield(a, b) }", env.clone());
        assert_eq!(run_env("call_block(3, 4) { |x, y| x + y }", env.clone()), Value::Int(7));
    }

    #[test]
    fn test_yield_in_loop() {
        // Implement a custom each using yield
        // Note: `while i < list.length { }` would cause the parser to treat `{` as a block
        // for `length`, so we store the length in a variable first.
        let env = Environment::new();
        exec_env("def my_each(list) { len = list.length; i = 0; while i < len { yield(list[i]); i = i + 1 } }", env.clone());
        exec_env("sum = 0; my_each([1, 2, 3]) { |x| sum = sum + x }", env.clone());
        assert_eq!(env.borrow().get("sum"), Some(Value::Int(6)));
    }

    #[test]
    fn test_yield_in_method() {
        let env = global_env();
        exec_env("class Wrapper { attr items; def each() { len = self.items.length; i = 0; while i < len { yield(self.items[i]); i = i + 1 } } }", env.clone());
        exec_env("w = Wrapper.new(items: [10, 20, 30])", env.clone());
        exec_env("sum = 0; w.each() { |x| sum = sum + x }", env.clone());
        assert_eq!(env.borrow().get("sum"), Some(Value::Int(60)));
    }

    #[test]
    fn test_reserved_class_cannot_be_redefined() {
        let env = global_env();
        let tokens = crate::lexer::Lexer::new("class List { attr x }").scan_tokens();
        let mut stmts = crate::parser::Parser::new(tokens).parse().unwrap();
        assert!(execute(stmts.remove(0), env).is_err());
    }

    #[test]
    fn test_map_delete() {
        let env = Environment::new();
        exec_env(r#"m = { name: "Alice", age: 30 }"#, env.clone());
        exec_env(r#"m.delete("name")"#, env.clone());
        assert_eq!(run_env("m.length", env.clone()), Value::Int(1));
        assert_eq!(run_env(r#"m.has_key?("name")"#, env.clone()), Value::Bool(false));
    }

    #[test]
    fn test_map_merge() {
        let env = Environment::new();
        exec_env(r#"a = { x: 1 }; b = { y: 2 }"#, env.clone());
        exec_env("c = a.merge(b)", env.clone());
        assert_eq!(run_env("c.length", env.clone()), Value::Int(2));
        assert_eq!(run_env(r#"c["x"]"#, env.clone()), Value::Int(1));
        assert_eq!(run_env(r#"c["y"]"#, env.clone()), Value::Int(2));
    }

    #[test]
    fn test_map_select() {
        let env = global_env();
        exec_env(r#"m = { a: 1, b: 2, c: 3 }; result = m.select { |k, v| v > 1 }"#, env.clone());
        assert_eq!(run_env("result.length", env.clone()), Value::Int(2));
        assert_eq!(run_env(r#"result.has_key?("a")"#, env.clone()), Value::Bool(false));
        assert_eq!(run_env(r#"result.has_key?("b")"#, env.clone()), Value::Bool(true));
    }

    #[test]
    fn test_map_any() {
        let env = global_env();
        exec_env(r#"m = { a: 1, b: 2 }"#, env.clone());
        assert_eq!(run_env("m.any? { |k, v| v > 1 }", env.clone()), Value::Bool(true));
        assert_eq!(run_env("m.any? { |k, v| v > 9 }", env.clone()), Value::Bool(false));
    }

    #[test]
    fn test_map_all() {
        let env = global_env();
        exec_env(r#"m = { a: 1, b: 2 }"#, env.clone());
        assert_eq!(run_env("m.all? { |k, v| v > 0 }", env.clone()), Value::Bool(true));
        assert_eq!(run_env("m.all? { |k, v| v > 1 }", env.clone()), Value::Bool(false));
    }

    #[test]
    fn test_map_none() {
        let env = global_env();
        exec_env(r#"m = { a: 1, b: 2 }"#, env.clone());
        assert_eq!(run_env("m.none? { |k, v| v > 9 }", env.clone()), Value::Bool(true));
        assert_eq!(run_env("m.none? { |k, v| v > 1 }", env.clone()), Value::Bool(false));
    }

    #[test]
    fn test_reserved_map_cannot_be_redefined() {
        let env = global_env();
        let tokens = crate::lexer::Lexer::new("class Map { attr x }").scan_tokens();
        let mut stmts = crate::parser::Parser::new(tokens).parse().unwrap();
        assert!(execute(stmts.remove(0), env).is_err());
    }

    #[test]
    fn test_implicit_self_field_read() {
        let env = global_env();
        exec_env("class Point { attr x; attr y; def sum() { x + y } }", env.clone());
        exec_env("p = Point.new(x: 3, y: 4)", env.clone());
        assert_eq!(run_env("p.sum()", env.clone()), Value::Int(7));
    }

    #[test]
    fn test_implicit_self_method_call() {
        let env = global_env();
        exec_env("class Counter { attr count; def increment() { self.count = count + 1 }; def value() { count } }", env.clone());
        exec_env("c = Counter.new(count: 0)", env.clone());
        exec_env("c.increment()", env.clone());
        exec_env("c.increment()", env.clone());
        assert_eq!(run_env("c.value()", env.clone()), Value::Int(2));
    }

    #[test]
    fn test_implicit_self_local_shadows_field() {
        let env = global_env();
        exec_env("class Box { attr x; def doubled() { x = 99; x } }", env.clone());
        exec_env("b = Box.new(x: 10)", env.clone());
        // local x = 99 should shadow self.x inside the method
        assert_eq!(run_env("b.doubled()", env.clone()), Value::Int(99));
        // self.x should be unchanged
        assert_eq!(run_env("b.x", env.clone()), Value::Int(10));
    }

    #[test]
    fn test_elsif_second_branch() {
        let env = Environment::new();
        exec_env("x = 5; result = 0\nif x > 10 { result = 1 } elsif x > 3 { result = 2 } else { result = 3 }", env.clone());
        assert_eq!(env.borrow().get("result"), Some(Value::Int(2)));
    }

    #[test]
    fn test_elsif_else_branch() {
        let env = Environment::new();
        exec_env("x = 1; result = 0\nif x > 10 { result = 1 } elsif x > 3 { result = 2 } else { result = 3 }", env.clone());
        assert_eq!(env.borrow().get("result"), Some(Value::Int(3)));
    }

    #[test]
    fn test_elsif_first_branch() {
        let env = Environment::new();
        exec_env("x = 20; result = 0\nif x > 10 { result = 1 } elsif x > 3 { result = 2 } else { result = 3 }", env.clone());
        assert_eq!(env.borrow().get("result"), Some(Value::Int(1)));
    }

    #[test]
    fn test_elsif_chain() {
        let env = Environment::new();
        exec_env("x = 5; result = 0\nif x == 1 { result = 1 } elsif x == 2 { result = 2 } elsif x == 5 { result = 5 } else { result = 99 }", env.clone());
        assert_eq!(env.borrow().get("result"), Some(Value::Int(5)));
    }

    #[test]
    fn test_defp_callable_from_within_class() {
        let env = global_env();
        exec_env("class Foo { attr x; defp secret() { x + 1 }; def pub() { secret() } }", env.clone());
        exec_env("f = Foo.new(x: 10)", env.clone());
        assert_eq!(run_env("f.pub()", env.clone()), Value::Int(11));
    }

    #[test]
    fn test_defp_blocked_from_outside() {
        let env = global_env();
        exec_env("class Foo { defp secret() { 42 } }", env.clone());
        exec_env("f = Foo.new()", env.clone());
        let tokens = crate::lexer::Lexer::new("f.secret()").scan_tokens();
        let mut stmts = crate::parser::Parser::new(tokens).parse().unwrap();
        assert!(execute(stmts.remove(0), env).is_err());
    }

    #[test]
    fn test_defp_inherited_callable_from_subclass() {
        let env = global_env();
        exec_env("class A { defp helper() { 99 }; def run() { helper() } }", env.clone());
        exec_env("class B < A { }", env.clone());
        exec_env("b = B.new()", env.clone());
        assert_eq!(run_env("b.run()", env.clone()), Value::Int(99));
    }

    #[test]
    fn test_downto() {
        let env = global_env();
        exec_env("result = []; 3.downto(1) { |i| result.push(i) }", env.clone());
        assert_eq!(run_env("result[0]", env.clone()), Value::Int(3));
        assert_eq!(run_env("result[2]", env.clone()), Value::Int(1));
    }

    #[test]
    fn test_string_chars() {
        let env = global_env();
        exec_env(r#"result = "hi".chars"#, env.clone());
        assert_eq!(run_env("result.length", env.clone()), Value::Int(2));
        assert_eq!(run_env("result[0]", env.clone()), Value::Str("h".into()));
        assert_eq!(run_env("result[1]", env.clone()), Value::Str("i".into()));
    }

    #[test]
    fn test_multi_assign_basic() {
        let env = Environment::new();
        exec_env("a, b = 1, 2", env.clone());
        assert_eq!(env.borrow().get("a"), Some(Value::Int(1)));
        assert_eq!(env.borrow().get("b"), Some(Value::Int(2)));
    }

    #[test]
    fn test_multi_assign_swap() {
        let env = Environment::new();
        exec_env("a = 1; b = 2", env.clone());
        exec_env("a, b = b, a", env.clone());
        assert_eq!(env.borrow().get("a"), Some(Value::Int(2)));
        assert_eq!(env.borrow().get("b"), Some(Value::Int(1)));
    }

    #[test]
    fn test_multi_assign_three() {
        let env = Environment::new();
        exec_env("x, y, z = 10, 20, 30", env.clone());
        assert_eq!(env.borrow().get("x"), Some(Value::Int(10)));
        assert_eq!(env.borrow().get("y"), Some(Value::Int(20)));
        assert_eq!(env.borrow().get("z"), Some(Value::Int(30)));
    }

    #[test]
    fn test_while_condition_method_call_no_block_greed() {
        // Previously `while i < list.length { }` caused the parser to attach
        // `{` as a block to `length`, breaking the loop. This must now parse correctly.
        let env = global_env();
        exec_env("list = [1, 2, 3]; i = 0; sum = 0; while i < list.length { sum = sum + list[i]; i = i + 1 }", env.clone());
        assert_eq!(env.borrow().get("sum"), Some(Value::Int(6)));
    }

    #[test]
    fn test_raise_unhandled() {
        let tokens = crate::lexer::Lexer::new(r#"raise "oops""#).scan_tokens();
        let mut stmts = crate::parser::Parser::new(tokens).parse().unwrap();
        let result = execute(stmts.remove(0), Environment::new());
        assert!(matches!(result, Err(SapphireError::Raised(Value::Str(_)))));
    }

    #[test]
    fn test_begin_rescue_catches_raise() {
        let env = Environment::new();
        exec_env(r#"x = 0; begin; raise "err"; rescue e; x = 1; end"#, env.clone());
        assert_eq!(env.borrow().get("x"), Some(Value::Int(1)));
    }

    #[test]
    fn test_begin_rescue_binds_message() {
        let env = Environment::new();
        exec_env(r#"begin; raise "boom"; rescue e; end"#, env.clone());
        assert_eq!(env.borrow().get("e"), Some(Value::Str("boom".into())));
    }

    #[test]
    fn test_begin_rescue_catches_runtime_error() {
        let env = Environment::new();
        exec_env("x = 0; begin; x = 1 / 0; rescue e; x = 99; end", env.clone());
        assert_eq!(env.borrow().get("x"), Some(Value::Int(99)));
    }

    #[test]
    fn test_begin_else_runs_when_no_error() {
        let env = Environment::new();
        exec_env("x = 0\nbegin\n  x = 1\nrescue e\n  x = 99\nelse\n  x = 2\nend", env.clone());
        assert_eq!(env.borrow().get("x"), Some(Value::Int(2)));
    }

    #[test]
    fn test_begin_else_skipped_on_error() {
        let env = Environment::new();
        exec_env("x = 0\nbegin\n  raise \"err\"\nrescue e\n  x = 99\nelse\n  x = 2\nend", env.clone());
        assert_eq!(env.borrow().get("x"), Some(Value::Int(99)));
    }

    #[test]
    fn test_begin_no_error_skips_rescue() {
        let env = Environment::new();
        exec_env("x = 0; begin; x = 42; rescue e; x = 1; end", env.clone());
        assert_eq!(env.borrow().get("x"), Some(Value::Int(42)));
    }

    #[test]
    fn test_inline_rescue_in_function() {
        let env = Environment::new();
        exec_env("def risky(x) { raise \"bad\" if x < 0\n x * 2\nrescue e\n 0 }", env.clone());
        assert_eq!(run_env("risky(5)", env.clone()), Value::Int(10));
        assert_eq!(run_env("risky(-1)", env.clone()), Value::Int(0));
    }

    #[test]
    fn test_inline_rescue_binds_error() {
        let env = Environment::new();
        exec_env("def boom() { raise \"oops\"\n 1\nrescue e\n e }", env.clone());
        assert_eq!(run_env("boom()", env.clone()), Value::Str("oops".into()));
    }

    #[test]
    fn test_inline_rescue_in_method() {
        let env = global_env();
        exec_env("class Safe { def try_div(x) { 10 / x\nrescue e\n -1 } }", env.clone());
        exec_env("s = Safe.new()", env.clone());
        assert_eq!(run_env("s.try_div(2)", env.clone()), Value::Int(5));
        assert_eq!(run_env("s.try_div(0)", env.clone()), Value::Int(-1));
    }

    #[test]
    fn test_raise_instance() {
        let env = global_env();
        exec_env("class Err { attr msg }; begin; raise Err.new(msg: \"bad\"); rescue e; end", env.clone());
        if let Some(Value::Instance { class_name, .. }) = env.borrow().get("e") {
            assert_eq!(class_name, "Err");
        } else {
            panic!("expected instance");
        }
    }

    #[test]
    fn test_object_class_registered() {
        let env = global_env();
        assert!(matches!(env.borrow().get("Object"), Some(Value::Class { .. })));
    }

    #[test]
    fn test_object_cannot_be_redefined() {
        let env = global_env();
        let tokens = crate::lexer::Lexer::new("class Object {}").scan_tokens();
        let mut stmts = crate::parser::Parser::new(tokens).parse().unwrap();
        assert!(execute(stmts.remove(0), env).is_err());
    }

    #[test]
    fn test_implicit_object_superclass() {
        let env = global_env();
        exec_env("class Animal { attr name }", env.clone());
        if let Some(Value::Class { superclass, .. }) = env.borrow().get("Animal") {
            assert_eq!(superclass, Some("Object".to_string()));
        } else {
            panic!("Animal not found");
        }
    }

    #[test]
    fn test_is_a_direct_class() {
        let env = global_env();
        exec_env("class Dog { attr name }; d = Dog.new(name: \"Rex\")", env.clone());
        assert_eq!(run_env("d.is_a?(\"Dog\")", env.clone()), Value::Bool(true));
    }

    #[test]
    fn test_is_a_superclass() {
        let env = global_env();
        exec_env("class Animal { attr name }; class Dog < Animal { attr breed }; d = Dog.new(name: \"Rex\", breed: \"Lab\")", env.clone());
        assert_eq!(run_env("d.is_a?(\"Animal\")", env.clone()), Value::Bool(true));
    }

    #[test]
    fn test_is_a_object() {
        let env = global_env();
        exec_env("class Foo {}; f = Foo.new()", env.clone());
        assert_eq!(run_env("f.is_a?(\"Object\")", env.clone()), Value::Bool(true));
    }

    #[test]
    fn test_is_a_unrelated_class() {
        let env = global_env();
        exec_env("class Cat {}; class Dog {}; d = Dog.new()", env.clone());
        assert_eq!(run_env("d.is_a?(\"Cat\")", env.clone()), Value::Bool(false));
    }

    #[test]
    fn test_is_a_deep_chain() {
        let env = global_env();
        exec_env("class A {}; class B < A {}; class C < B {}; c = C.new()", env.clone());
        assert_eq!(run_env("c.is_a?(\"C\")", env.clone()), Value::Bool(true));
        assert_eq!(run_env("c.is_a?(\"B\")", env.clone()), Value::Bool(true));
        assert_eq!(run_env("c.is_a?(\"A\")", env.clone()), Value::Bool(true));
        assert_eq!(run_env("c.is_a?(\"Object\")", env.clone()), Value::Bool(true));
    }

    #[test]
    fn test_super_still_works_with_implicit_object() {
        let env = global_env();
        exec_env("class Animal { attr name; def speak() { \"...\" } }", env.clone());
        exec_env("class Dog < Animal { def speak() { super.speak() } }", env.clone());
        exec_env("d = Dog.new(name: \"Rex\")", env.clone());
        assert_eq!(run_env("d.speak()", env.clone()), Value::Str("...".into()));
    }

    #[test]
    fn test_range_literal() {
        assert_eq!(run("1..10"), Value::Range { from: 1, to: 10 });
    }

    #[test]
    fn test_range_each() {
        let env = global_env();
        exec_env("sum = 0; (1..5).each { |i| sum = sum + i }", env.clone());
        assert_eq!(env.borrow().get("sum"), Some(Value::Int(15)));
    }

    #[test]
    fn test_range_include() {
        let env = global_env();
        assert_eq!(run_env("(1..10).include?(5)", env.clone()), Value::Bool(true));
        assert_eq!(run_env("(1..10).include?(11)", env.clone()), Value::Bool(false));
        assert_eq!(run_env("(1..10).include?(1)", env.clone()), Value::Bool(true));
        assert_eq!(run_env("(1..10).include?(10)", env.clone()), Value::Bool(true));
    }

    #[test]
    fn test_range_to_s() {
        assert_eq!(run("(1..5).to_s"), Value::Str("1..5".into()));
    }

    #[test]
    fn test_float_literal() {
        assert_eq!(run("3.14"), Value::Float(3.14));
        assert_eq!(run("1.0"), Value::Float(1.0));
    }

    #[test]
    fn test_float_arithmetic() {
        assert_eq!(run("1.5 + 2.5"), Value::Float(4.0));
        assert_eq!(run("3.0 - 1.5"), Value::Float(1.5));
        assert_eq!(run("2.0 * 3.0"), Value::Float(6.0));
        assert_eq!(run("7.0 / 2.0"), Value::Float(3.5));
    }

    #[test]
    fn test_float_mixed_arithmetic() {
        assert_eq!(run("1 + 0.5"), Value::Float(1.5));
        assert_eq!(run("0.5 + 1"), Value::Float(1.5));
        assert_eq!(run("3 * 1.5"), Value::Float(4.5));
        assert_eq!(run("7 / 2.0"), Value::Float(3.5));
    }

    #[test]
    fn test_int_division_stays_int() {
        assert_eq!(run("7 / 2"), Value::Int(3));
    }

    #[test]
    fn test_float_comparison() {
        assert_eq!(run("1.5 < 2.0"), Value::Bool(true));
        assert_eq!(run("2.0 > 1.5"), Value::Bool(true));
        assert_eq!(run("1.0 == 1.0"), Value::Bool(true));
        assert_eq!(run("1.0 == 1"), Value::Bool(true));
        assert_eq!(run("1 == 1.0"), Value::Bool(true));
    }

    #[test]
    fn test_float_negation() {
        assert_eq!(run("-3.14"), Value::Float(-3.14));
    }

    #[test]
    fn test_float_to_i() {
        assert_eq!(run("3.9.to_i"), Value::Int(3));
        assert_eq!(run("-3.9.to_i"), Value::Int(-3));
    }

    #[test]
    fn test_int_to_f() {
        assert_eq!(run("3.to_f"), Value::Float(3.0));
    }

    #[test]
    fn test_float_to_s() {
        assert_eq!(run("3.14.to_s"), Value::Str("3.14".into()));
        assert_eq!(run("1.0.to_s"), Value::Str("1.0".into()));
    }

    #[test]
    fn test_string_escape_newline() {
        assert_eq!(run(r#""\n""#), Value::Str("\n".into()));
    }

    #[test]
    fn test_string_escape_tab() {
        assert_eq!(run(r#""\t""#), Value::Str("\t".into()));
    }

    #[test]
    fn test_string_escape_backslash() {
        assert_eq!(run(r#""\\""#), Value::Str("\\".into()));
    }

    #[test]
    fn test_string_escape_quote() {
        assert_eq!(run(r#""\"""#), Value::Str("\"".into()));
    }

    #[test]
    fn test_string_escape_in_interpolation() {
        // \n inside an interpolated string
        assert_eq!(run(r#""a\nb""#), Value::Str("a\nb".into()));
    }

    #[test]
    fn test_list_first_no_arg() {
        assert_eq!(run("[1, 2, 3].first()"), Value::Int(1));
    }

    #[test]
    fn test_list_first_n() {
        let env = global_env();
        exec_env("result = [1, 2, 3, 4, 5].first(3)", env.clone());
        assert_eq!(run_env("result.length", env.clone()), Value::Int(3));
        assert_eq!(run_env("result[0]", env.clone()), Value::Int(1));
        assert_eq!(run_env("result[2]", env.clone()), Value::Int(3));
    }

    #[test]
    fn test_list_last_no_arg() {
        assert_eq!(run("[1, 2, 3].last()"), Value::Int(3));
    }

    #[test]
    fn test_list_last_n() {
        let env = global_env();
        exec_env("result = [1, 2, 3, 4, 5].last(2)", env.clone());
        assert_eq!(run_env("result.length", env.clone()), Value::Int(2));
        assert_eq!(run_env("result[0]", env.clone()), Value::Int(4));
        assert_eq!(run_env("result[1]", env.clone()), Value::Int(5));
    }

    #[test]
    fn test_list_count_no_block() {
        assert_eq!(run("[1, 2, 3].count()"), Value::Int(3));
    }

    #[test]
    fn test_list_count_with_block() {
        let env = global_env();
        assert_eq!(run_env("[1, 2, 3, 4].count { |x| x > 2 }", env), Value::Int(2));
    }

    #[test]
    fn test_list_sort() {
        let env = global_env();
        exec_env("result = [3, 1, 4, 1, 5, 9, 2].sort()", env.clone());
        assert_eq!(run_env("result[0]", env.clone()), Value::Int(1));
        assert_eq!(run_env("result[1]", env.clone()), Value::Int(1));
        assert_eq!(run_env("result[6]", env.clone()), Value::Int(9));
    }

    #[test]
    fn test_list_sort_strings() {
        let env = global_env();
        exec_env(r#"result = ["banana", "apple", "cherry"].sort()"#, env.clone());
        assert_eq!(run_env("result[0]", env.clone()), Value::Str("apple".into()));
        assert_eq!(run_env("result[2]", env.clone()), Value::Str("cherry".into()));
    }

    #[test]
    fn test_list_sort_by() {
        let env = global_env();
        exec_env(r#"words = ["banana", "fig", "apple"]; result = words.sort_by { |w| w.length }"#, env.clone());
        assert_eq!(run_env("result[0]", env.clone()), Value::Str("fig".into()));
        assert_eq!(run_env("result[2]", env.clone()), Value::Str("banana".into()));
    }

    #[test]
    fn test_list_flatten() {
        let env = global_env();
        exec_env("result = [[1, 2], [3, [4, 5]]].flatten()", env.clone());
        assert_eq!(run_env("result.length", env.clone()), Value::Int(5));
        assert_eq!(run_env("result[3]", env.clone()), Value::Int(4));
    }

    #[test]
    fn test_list_uniq() {
        let env = global_env();
        exec_env("result = [1, 2, 2, 3, 1].uniq()", env.clone());
        assert_eq!(run_env("result.length", env.clone()), Value::Int(3));
        assert_eq!(run_env("result[0]", env.clone()), Value::Int(1));
        assert_eq!(run_env("result[2]", env.clone()), Value::Int(3));
    }

    #[test]
    fn test_list_each_with_index() {
        let env = global_env();
        exec_env("pairs = []; [\"a\", \"b\", \"c\"].each_with_index { |item, i| pairs.push(i) }", env.clone());
        assert_eq!(run_env("pairs[0]", env.clone()), Value::Int(0));
        assert_eq!(run_env("pairs[1]", env.clone()), Value::Int(1));
        assert_eq!(run_env("pairs[2]", env.clone()), Value::Int(2));
    }

    #[test]
    fn test_list_include() {
        let env = global_env();
        assert_eq!(run_env("[1, 2, 3].include?(2)", env.clone()), Value::Bool(true));
        assert_eq!(run_env("[1, 2, 3].include?(9)", env.clone()), Value::Bool(false));
    }

    #[test]
    fn test_list_zip() {
        let env = global_env();
        exec_env("result = [1, 2, 3].zip([4, 5, 6])", env.clone());
        assert_eq!(run_env("result.length", env.clone()), Value::Int(3));
        // result[0] should be [1, 4]
        if let Value::List(pair) = run_env("result[0]", env.clone()) {
            assert_eq!(pair.borrow()[0], Value::Int(1));
            assert_eq!(pair.borrow()[1], Value::Int(4));
        } else {
            panic!("expected List");
        }
    }
}
