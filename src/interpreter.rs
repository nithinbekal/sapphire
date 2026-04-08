use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;
use std::io::{self, Write};
use std::sync::atomic::{AtomicU64, Ordering};

static FRAME_COUNTER: AtomicU64 = AtomicU64::new(1);
fn next_frame_id() -> u64 { FRAME_COUNTER.fetch_add(1, Ordering::Relaxed) }
use crate::ast::{Block, Expr, MethodDef, StringPart, TypeExpr};
use crate::environment::Environment;
use crate::value::EnvRef;
use crate::error::SapphireError;
use crate::token::TokenKind;
use crate::value::Value;

const OBJECT_STDLIB: &str = include_str!("../stdlib/object.spr");
const NIL_STDLIB: &str = include_str!("../stdlib/nil.spr");
const NUM_STDLIB: &str = include_str!("../stdlib/num.spr");
const INT_STDLIB: &str = include_str!("../stdlib/int.spr");
const FLOAT_STDLIB: &str = include_str!("../stdlib/float.spr");
const STRING_STDLIB: &str = include_str!("../stdlib/string.spr");
const BOOL_STDLIB: &str = include_str!("../stdlib/bool.spr");
const LIST_STDLIB: &str = include_str!("../stdlib/list.spr");
const MAP_STDLIB: &str = include_str!("../stdlib/map.spr");

fn check_type(value: &Value, te: &TypeExpr) -> bool {
    match te {
        TypeExpr::Any => true,
        TypeExpr::Named(name) => match name.as_str() {
            "Int"    => matches!(value, Value::Int(_)),
            "Float"  => matches!(value, Value::Float(_)),
            "Num"    => matches!(value, Value::Int(_) | Value::Float(_)),
            "String" => matches!(value, Value::Str(_)),
            "Bool"   => matches!(value, Value::Bool(_)),
            "Nil"    => matches!(value, Value::Nil),
            "List"   => matches!(value, Value::List(_)),
            "Map"    => matches!(value, Value::Map(_)),
            class_name => matches!(value, Value::Instance { class_name: cn, .. } if cn == class_name),
        },
    }
}

fn value_type_description(value: &Value) -> String {
    match value {
        Value::Int(_)                      => "Int".to_string(),
        Value::Float(_)                    => "Float".to_string(),
        Value::Str(_)                      => "String".to_string(),
        Value::Bool(_)                     => "Bool".to_string(),
        Value::Nil                         => "Nil".to_string(),
        Value::List(_)                     => "List".to_string(),
        Value::Map(_)                      => "Map".to_string(),
        Value::Instance { class_name, .. } => class_name.clone(),
        _                                  => "unknown".to_string(),
    }
}

fn type_expr_name(te: &TypeExpr) -> &str {
    match te {
        TypeExpr::Named(n) => n.as_str(),
        TypeExpr::Any => "Any",
    }
}

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
    for (src, label) in [
        (OBJECT_STDLIB, "stdlib/object.spr"),
        (NIL_STDLIB,    "stdlib/nil.spr"),
        (NUM_STDLIB,    "stdlib/num.spr"),
        (INT_STDLIB,    "stdlib/int.spr"),
        (FLOAT_STDLIB,  "stdlib/float.spr"),
        (STRING_STDLIB, "stdlib/string.spr"),
        (BOOL_STDLIB,   "stdlib/bool.spr"),
        (LIST_STDLIB,   "stdlib/list.spr"),
        (MAP_STDLIB,    "stdlib/map.spr"),
    ] {
        let tokens = crate::lexer::Lexer::new(src).scan_tokens();
        let stmts = crate::parser::Parser::new(tokens).parse()
            .unwrap_or_else(|e| panic!("{} failed to parse: {}", label, e));
        for expr in stmts {
            execute(expr, env.clone())
                .unwrap_or_else(|e| panic!("{} failed to execute: {}", label, e));
        }
    }
    env.borrow_mut().freeze("Object");
    env.borrow_mut().freeze("Nil");
    env.borrow_mut().freeze("Num");
    env.borrow_mut().freeze("Int");
    env.borrow_mut().freeze("Float");
    env.borrow_mut().freeze("String");
    env.borrow_mut().freeze("Bool");
    env.borrow_mut().freeze("List");
    env.borrow_mut().freeze("Map");
    // Establish an implicit top-level 'self' (main), an instance of Object,
    // so that bare calls to top-level methods resolve via the implicit-self fallback.
    let main_obj = Value::Instance {
        class_name: "Object".to_string(),
        fields: Rc::new(RefCell::new(HashMap::new())),
    };
    env.borrow_mut().set("self".to_string(), main_obj);
    env
}

pub fn execute(expr: Expr, env: EnvRef) -> Result<Option<Value>, SapphireError> {
    match expr {
        Expr::Return(e) => {
            let value = evaluate(*e, env.clone())?;
            let frame_id = match env.borrow().get("__frame_id__") {
                Some(Value::Int(id)) => id as u64,
                _ => return Err(SapphireError::RuntimeError {
                    message: "return called outside of a method".into(),
                }),
            };
            Err(SapphireError::NonLocalReturn(value, frame_id))
        }
        Expr::Break(e) => {
            let value = evaluate(*e, env)?;
            Err(SapphireError::Break(value))
        }
        Expr::Next(e) => {
            let value = evaluate(*e, env)?;
            Err(SapphireError::Next(value))
        }
        Expr::MultiAssign { names, values } => {
            if names.len() != values.len() {
                return Err(SapphireError::RuntimeError {
                    message: format!("expected {} value(s), got {}", names.len(), values.len()),
                });
            }
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
        Expr::Raise(e) => {
            let value = evaluate(*e, env)?;
            Err(SapphireError::Raised(value))
        }
        Expr::While { condition, body } => {
            'while_loop: loop {
                let cond = evaluate((*condition).clone(), env.clone())?;
                match cond {
                    Value::Bool(true) => {
                        for e in body.clone() {
                            match execute(e, env.clone()) {
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
        e => Ok(Some(evaluate_impl(e, env)?)),
    }
}

pub fn evaluate(expr: Expr, env: EnvRef) -> Result<Value, SapphireError> {
    match execute(expr, env)? {
        Some(v) => Ok(v),
        None => Ok(Value::Nil),
    }
}

fn evaluate_impl(expr: Expr, env: EnvRef) -> Result<Value, SapphireError> {
    match expr {
        Expr::Return(_)
        | Expr::Break(_)
        | Expr::Next(_)
        | Expr::Raise(_)
        | Expr::While { .. }
        | Expr::MultiAssign { .. } => match execute(expr, env)? {
            Some(v) => Ok(v),
            None => Ok(Value::Nil),
        },
        Expr::Lambda { params, body } => Ok(Value::Lambda { params, body, closure: env }),
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
            let is_constant = name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
                && name.chars().all(|c| c.is_uppercase() || c.is_ascii_digit() || c == '_');
            if is_constant && env.borrow().is_frozen(&name) {
                return Err(SapphireError::RuntimeError {
                    message: format!("cannot reassign constant '{}'", name),
                });
            }
            let result = evaluate(*value, env.clone())?;
            if !env.borrow_mut().assign(&name, result.clone()) {
                env.borrow_mut().set(name.clone(), result.clone());
            }
            if is_constant {
                env.borrow_mut().freeze(&name);
            }
            Ok(result)
        }
        Expr::SelfExpr => env.borrow().get("self").ok_or_else(|| SapphireError::RuntimeError {
            message: "self used outside of a method".into(),
        }),
        Expr::Get { object, name } => {
            let obj = evaluate(*object, env.clone())?;

            // Nil: dispatch to Nil stdlib class
            if obj == Value::Nil {
                if name == "class" {
                    return env.borrow().get("Nil").ok_or_else(|| SapphireError::RuntimeError {
                        message: "class 'Nil' not found".into(),
                    });
                }
                if name == "is_a?" {
                    return Ok(Value::NativeMethod { receiver: Box::new(obj), name });
                }
                if let Some(Value::Class { name: class_name, methods, closure, .. }) = env.borrow().get("Nil") {
                    if let Some(method) = methods.iter().find(|m| m.name == name) {
                        return Ok(Value::BoundMethod {
                            receiver: Box::new(obj),
                            params: method.params.clone(),
                            return_type: method.return_type.clone(),
                            body: method.body.clone(),
                            closure,
                            defined_in: class_name,
                        });
                    }
                }
                return Err(SapphireError::RuntimeError {
                    message: format!("undefined method '{}' on nil", name),
                });
            }

            // Universal built-ins — checked first so they work on every type
            // (instances can override these by defining a method of the same name)
            if !matches!(obj, Value::Instance { .. }) {
                match name.as_str() {
                    "nil?" => return Ok(Value::Bool(false)),
                    "class" => {
                        let cn = value_type_description(&obj);
                        return env.borrow().get(&cn).ok_or_else(|| SapphireError::RuntimeError {
                            message: format!("class '{}' not found", cn),
                        });
                    }
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
                    "is_a?" => return Ok(Value::NativeMethod {
                        receiver: Box::new(obj),
                        name,
                    }),
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
                            return_type: method.return_type.clone(),
                            body: method.body.clone(),
                            closure,
                            defined_in: cname,
                        });
                    }
                }
                // Built-in fallbacks for instances
                return match name.as_str() {
                    "class" => env.borrow().get(class_name).ok_or_else(|| SapphireError::RuntimeError {
                        message: format!("class '{}' not found", class_name),
                    }),
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
                            return_type: method.return_type.clone(),
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
                            return_type: method.return_type.clone(),
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
                match name.as_str() {
                    "downto" => return Ok(Value::NativeMethod {
                        receiver: Box::new(Value::Int(n)),
                        name,
                    }),
                    _ => {}
                }
                if let Some(Value::Class { name: class_name, methods, closure, .. }) = env.borrow().get("Int") {
                    if let Some(method) = methods.iter().find(|m| m.name == name) {
                        return Ok(Value::BoundMethod {
                            receiver: Box::new(Value::Int(n)),
                            params: method.params.clone(),
                            return_type: method.return_type.clone(),
                            body: method.body.clone(),
                            closure,
                            defined_in: class_name,
                        });
                    }
                }
                return Err(SapphireError::RuntimeError {
                    message: format!("undefined method '{}' on Int", name),
                });
            }

            // Floats
            if let Value::Float(f) = obj {
                if let Some(Value::Class { name: class_name, methods, closure, .. }) = env.borrow().get("Float") {
                    if let Some(method) = methods.iter().find(|m| m.name == name) {
                        return Ok(Value::BoundMethod {
                            receiver: Box::new(Value::Float(f)),
                            params: method.params.clone(),
                            return_type: method.return_type.clone(),
                            body: method.body.clone(),
                            closure,
                            defined_in: class_name,
                        });
                    }
                }
                return Err(SapphireError::RuntimeError {
                    message: format!("undefined method '{}' on Float", name),
                });
            }

            // Booleans
            if let Value::Bool(b) = obj {
                if let Some(Value::Class { name: class_name, methods, closure, .. }) = env.borrow().get("Bool") {
                    if let Some(method) = methods.iter().find(|m| m.name == name) {
                        return Ok(Value::BoundMethod {
                            receiver: Box::new(Value::Bool(b)),
                            params: method.params.clone(),
                            return_type: method.return_type.clone(),
                            body: method.body.clone(),
                            closure,
                            defined_in: class_name,
                        });
                    }
                }
                return Err(SapphireError::RuntimeError {
                    message: format!("undefined method '{}' on Bool", name),
                });
            }

            // Strings
            if let Value::Str(ref s) = obj {
                match name.as_str() {
                    "length" => return Ok(Value::Int(s.chars().count() as i64)),
                    "upcase"   => return Ok(Value::Str(s.to_uppercase())),
                    "downcase" => return Ok(Value::Str(s.to_lowercase())),
                    "strip"    => return Ok(Value::Str(s.trim().to_string())),
                    "chomp"    => return Ok(Value::Str(s.trim_end_matches('\n').trim_end_matches('\r').to_string())),
                    "empty?"   => return Ok(Value::Bool(s.is_empty())),
                    "chars" => {
                        let chars: Vec<Value> = s.chars().map(|c| Value::Str(c.to_string())).collect();
                        return Ok(Value::List(Rc::new(RefCell::new(chars))));
                    }
                    "split" | "include?" | "starts_with?" | "ends_with?" => return Ok(Value::NativeMethod {
                        receiver: Box::new(obj.clone()),
                        name,
                    }),
                    _ => {}
                }
                if let Some(Value::Class { name: class_name, methods, closure, .. }) = env.borrow().get("String") {
                    if let Some(method) = methods.iter().find(|m| m.name == name) {
                        return Ok(Value::BoundMethod {
                            receiver: Box::new(obj.clone()),
                            params: method.params.clone(),
                            return_type: method.return_type.clone(),
                            body: method.body.clone(),
                            closure,
                            defined_in: class_name,
                        });
                    }
                }
                return Err(SapphireError::RuntimeError {
                    message: format!("undefined method '{}' on String", name),
                });
            }

            // Class: .new, .name
            if let Value::Class { name: class_name, fields, .. } = obj {
                return match name.as_str() {
                    "new"  => Ok(Value::Constructor { class_name, fields }),
                    "name" => Ok(Value::Str(class_name)),
                    _ => Err(SapphireError::RuntimeError {
                        message: format!("unknown class method '{}'", name),
                    }),
                };
            }

            // Lambda: only `.call` is valid
            if let Value::Lambda { .. } = &obj {
                if name == "call" {
                    return Ok(Value::NativeMethod { receiver: Box::new(obj), name });
                }
                return Err(SapphireError::RuntimeError {
                    message: format!("undefined method '{}' on lambda", name),
                });
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
            let val = evaluate(*value, env.clone())?;
            match obj {
                Value::Instance { class_name: ref cn, ref fields } => {
                    if let Some(Value::Class { fields: ref field_defs, .. }) = env.borrow().get(cn) {
                        if let Some(fd) = field_defs.iter().find(|f| f.name == name) {
                            if let Some(te) = &fd.type_ann {
                                if !check_type(&val, te) {
                                    return Err(SapphireError::TypeError {
                                        message: format!("field '{}' expected {}, got {}", name, type_expr_name(te), value_type_description(&val)),
                                    });
                                }
                            }
                        }
                    }
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
                Value::BoundMethod { receiver, params, return_type, body, closure, defined_in } => {
                    let arg_vals: Vec<Value> = eval_args.into_iter().map(|(_, v)| v).collect();
                    if params.len() != arg_vals.len() {
                        return Err(SapphireError::RuntimeError {
                            message: format!("expected {} argument(s), got {}", params.len(), arg_vals.len()),
                        });
                    }
                    let call_env = Environment::new_child(closure);
                    let frame_id = next_frame_id();
                    call_env.borrow_mut().set("__frame_id__".to_string(), Value::Int(frame_id as i64));
                    call_env.borrow_mut().set("self".to_string(), *receiver);
                    call_env.borrow_mut().set("__class__".to_string(), Value::Str(defined_in));
                    for (param, val) in params.iter().zip(arg_vals.iter()) {
                        if let Some(te) = &param.type_ann {
                            if !check_type(val, te) {
                                return Err(SapphireError::TypeError {
                                    message: format!("argument '{}' expected {}, got {}", param.name, type_expr_name(te), value_type_description(val)),
                                });
                            }
                        }
                        call_env.borrow_mut().set(param.name.clone(), val.clone());
                    }
                    if let Some(blk) = block {
                        call_env.borrow_mut().set("__block__".to_string(), Value::Block(blk, env.clone()));
                    }
                    let mut result = Value::Nil;
                    for stmt in body {
                        match execute(stmt, call_env.clone()) {
                            Ok(Some(v)) => result = v,
                            Ok(None) => {}
                            Err(SapphireError::NonLocalReturn(v, id)) if id == frame_id => {
                                if let Some(te) = &return_type {
                                    if !check_type(&v, te) {
                                        return Err(SapphireError::TypeError {
                                            message: format!("return value expected {}, got {}", type_expr_name(te), value_type_description(&v)),
                                        });
                                    }
                                }
                                return Ok(v);
                            }
                            Err(e) => return Err(e),
                        }
                    }
                    if let Some(te) = &return_type {
                        if !check_type(&result, te) {
                            return Err(SapphireError::TypeError {
                                message: format!("return value expected {}, got {}", type_expr_name(te), value_type_description(&result)),
                            });
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
                                if let Some(fd) = fields.iter().find(|f| f.name == n) {
                                    if let Some(te) = &fd.type_ann {
                                        if !check_type(&val, te) {
                                            return Err(SapphireError::TypeError {
                                                message: format!("field '{}' expected {}, got {}", n, type_expr_name(te), value_type_description(&val)),
                                            });
                                        }
                                    }
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
                        (receiver, "is_a?") => {
                            let target = match args.into_iter().next() {
                                Some(Value::Class { name, .. }) => name,
                                _ => return Err(SapphireError::RuntimeError {
                                    message: "is_a? requires a class argument".into(),
                                }),
                            };
                            let start = match receiver {
                                Value::Instance { class_name, .. } => class_name,
                                other => value_type_description(&other),
                            };
                            let mut current: Option<String> = Some(start);
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
                        (Value::Lambda { params, body, closure }, "call") => {
                            if params.len() != args.len() {
                                return Err(SapphireError::RuntimeError {
                                    message: format!("expected {} argument(s), got {}", params.len(), args.len()),
                                });
                            }
                            let lambda_env = Environment::new_child(closure);
                            let frame_id = next_frame_id();
                            lambda_env.borrow_mut().set("__frame_id__".to_string(), Value::Int(frame_id as i64));
                            for (param, val) in params.iter().zip(args.iter()) {
                                lambda_env.borrow_mut().set(param.clone(), val.clone());
                            }
                            let mut result = Value::Nil;
                            for expr in body {
                                match execute(expr, lambda_env.clone()) {
                                    Ok(Some(v)) => result = v,
                                    Ok(None) => {}
                                    Err(SapphireError::NonLocalReturn(v, id)) if id == frame_id => return Ok(v),
                                    Err(e) => return Err(e),
                                }
                            }
                            Ok(result)
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
        Expr::If { condition, then_branch, else_branch } => {
            let cond = evaluate(*condition, env.clone())?;
            let branch = match cond {
                Value::Bool(true)  => Some(then_branch),
                Value::Bool(false) => else_branch,
                _ => return Err(SapphireError::RuntimeError {
                    message: "if condition must be a boolean".into(),
                }),
            };
            match branch {
                None => Ok(Value::Nil),
                Some(stmts) => {
                    let mut result = Value::Nil;
                    for stmt in stmts {
                        match execute(stmt, env.clone())? {
                            Some(v) => result = v,
                            None => {}
                        }
                    }
                    Ok(result)
                }
            }
        }
        Expr::Begin {
            body,
            rescue_var,
            rescue_body,
            else_body,
        } => {
            let mut result = Value::Nil;
            let mut caught: Option<Value> = None;
            for stmt in body {
                match execute(stmt, env.clone()) {
                    Ok(Some(v)) => result = v,
                    Ok(None) => {}
                    Err(SapphireError::Raised(v)) => {
                        caught = Some(v);
                        break;
                    }
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
            Ok(result)
        }
        Expr::Print(inner) => {
            let value = evaluate(*inner, env)?;
            println!("{}", value);
            Ok(value)
        }
        Expr::Class { name, superclass, fields, methods } => {
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
            env.borrow_mut().set(name.clone(), class.clone());
            env.borrow_mut().freeze(&name);
            Ok(class)
        }
        Expr::Function { name, params, return_type, body } => {
            let method = MethodDef { name: name.clone(), params, return_type, body, private: false };
            let object_val = env.borrow().get("Object").ok_or_else(|| SapphireError::RuntimeError {
                message: "Object class not found".into(),
            })?;
            let updated = match object_val {
                Value::Class { name: cn, superclass, fields, mut methods, closure } => {
                    methods.retain(|m: &MethodDef| m.name != name);
                    methods.push(method);
                    Value::Class { name: cn, superclass, fields, methods, closure }
                }
                _ => return Err(SapphireError::RuntimeError { message: "Object is not a class".into() }),
            };
            env.borrow_mut().assign("Object", updated);
            Ok(Value::Str(name.clone()))
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
            let frame_id = next_frame_id();
            call_env.borrow_mut().set("__frame_id__".to_string(), Value::Int(frame_id as i64));
            call_env.borrow_mut().set("self".to_string(), self_val);
            call_env.borrow_mut().set("__class__".to_string(), Value::Str(super_name));
            for (param, val) in meth.params.iter().zip(arg_vals) {
                call_env.borrow_mut().set(param.name.clone(), val);
            }
            let _ = block; // super calls don't support blocks for now
            let mut result = Value::Nil;
            for stmt in meth.body.clone() {
                match execute(stmt, call_env.clone()) {
                    Ok(Some(v)) => result = v,
                    Ok(None) => {}
                    Err(SapphireError::NonLocalReturn(v, id)) if id == frame_id => return Ok(v),
                    Err(e) => return Err(e),
                }
            }
            Ok(result)
        }
    }
}

fn run_block(block: &Block, args: Vec<Value>, env: EnvRef) -> Result<Value, SapphireError> {
    let block_env = Environment::new_child(env);
    if block.params.is_empty() {
        if let Some(first) = args.first() {
            block_env.borrow_mut().set("it".to_string(), first.clone());
        }
    } else {
        for (param, val) in block.params.iter().zip(args) {
            block_env.borrow_mut().set(param.clone(), val);
        }
    }
    let mut result = Value::Nil;
    for stmt in &block.body {
        match execute(stmt.clone(), block_env.clone()) {
            Ok(Some(v)) => result = v,
            Ok(None) => {}
            Err(SapphireError::Next(v)) => return Ok(v),
            Err(e) => return Err(e),
        }
    }
    Ok(result)
}
