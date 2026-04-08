use std::collections::HashMap;
use crate::ast::{CallArg, Expr, FieldDef, ParamDef, TypeExpr};
use crate::token::TokenKind;
use crate::value::Value;

#[derive(Debug, Clone)]
pub struct TypeCheckError {
    pub message: String,
}

impl std::fmt::Display for TypeCheckError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "type error: {}", self.message)
    }
}

#[derive(Clone)]
struct FnSig {
    params: Vec<ParamDef>,
    return_type: Option<TypeExpr>,
}

#[derive(Clone)]
struct ClassInfo {
    fields: Vec<FieldDef>,
    methods: HashMap<String, FnSig>,
}

pub struct TypeChecker {
    functions: HashMap<String, FnSig>,
    classes: HashMap<String, ClassInfo>,
    errors: Vec<TypeCheckError>,
    current_return_type: Option<TypeExpr>,
    var_scopes: Vec<HashMap<String, TypeExpr>>,
}

impl TypeChecker {
    fn new() -> Self {
        Self {
            functions: HashMap::new(),
            classes: HashMap::new(),
            errors: Vec::new(),
            current_return_type: None,
            var_scopes: vec![HashMap::new()],
        }
    }

    pub fn check(exprs: &[Expr]) -> Vec<TypeCheckError> {
        let mut tc = Self::new();
        for e in exprs { tc.collect_def(e); }
        for e in exprs { tc.check_expr(e); }
        tc.errors
    }

    // First pass: record function and class signatures without checking bodies.
    fn collect_def(&mut self, expr: &Expr) {
        match expr {
            Expr::Function { name, params, return_type, .. } => {
                self.functions.insert(name.clone(), FnSig {
                    params: params.clone(),
                    return_type: return_type.clone(),
                });
            }
            Expr::Class { name, fields, methods, .. } => {
                let method_sigs = methods.iter().map(|m| {
                    (m.name.clone(), FnSig { params: m.params.clone(), return_type: m.return_type.clone() })
                }).collect();
                self.classes.insert(name.clone(), ClassInfo {
                    fields: fields.clone(),
                    methods: method_sigs,
                });
            }
            _ => {}
        }
    }

    fn push_scope(&mut self) { self.var_scopes.push(HashMap::new()); }
    fn pop_scope(&mut self)  { self.var_scopes.pop(); }

    fn set_var(&mut self, name: &str, ty: TypeExpr) {
        if let Some(scope) = self.var_scopes.last_mut() {
            scope.insert(name.to_string(), ty);
        }
    }

    fn get_var(&self, name: &str) -> Option<TypeExpr> {
        for scope in self.var_scopes.iter().rev() {
            if let Some(ty) = scope.get(name) { return Some(ty.clone()); }
        }
        None
    }

    fn check_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Return(inner) => {
                if let Some(rt) = self.current_return_type.clone() {
                    if let Some(actual) = self.infer_type(inner) {
                        if !types_compatible(&actual, &rt) {
                            self.errors.push(TypeCheckError {
                                message: format!("return value expected {}, got {}", te_name(&rt), te_name(&actual)),
                            });
                        }
                    }
                }
                self.check_expr(inner);
            }
            Expr::While { condition, body } => {
                self.check_expr(condition);
                self.push_scope();
                for s in body { self.check_expr(s); }
                self.pop_scope();
            }
            Expr::Raise(inner) => self.check_expr(inner),
            Expr::Break(inner) | Expr::Next(inner) => self.check_expr(inner),
            Expr::MultiAssign { names, values } => {
                for (name, ve) in names.iter().zip(values.iter()) {
                    if let Some(ty) = self.infer_type(ve) { self.set_var(name, ty); }
                    self.check_expr(ve);
                }
            }
            Expr::Call { callee, args, .. } => self.check_call(callee, args),
            Expr::Assign { name, value } => {
                let ty = self.infer_type(value).or_else(|| {
                    if let Expr::Call { callee, .. } = value.as_ref() {
                        if let Expr::Get { object, name: m } = callee.as_ref() {
                            if m == "new" {
                                if let Expr::Variable(cn) = object.as_ref() {
                                    if self.classes.contains_key(cn) {
                                        return Some(TypeExpr::Named(cn.clone()));
                                    }
                                }
                            }
                        }
                    }
                    None
                });
                if let Some(ty) = ty { self.set_var(name, ty); }
                self.check_expr(value);
            }
            Expr::Binary { left, right, .. } => { self.check_expr(left); self.check_expr(right); }
            Expr::Unary { right, .. } => self.check_expr(right),
            Expr::Get { object, .. } | Expr::SafeGet { object, .. } => self.check_expr(object),
            Expr::Set { object, value, name } => {
                // If we can determine the receiver's class, check the field type.
                if let Some(TypeExpr::Named(class_name)) = self.infer_type(object) {
                    if let Some(cls) = self.classes.get(&class_name).cloned() {
                        if let Some(fd) = cls.fields.iter().find(|f| &f.name == name) {
                            if let Some(te) = &fd.type_ann {
                                if let Some(actual) = self.infer_type(value) {
                                    if !types_compatible(&actual, te) {
                                        self.errors.push(TypeCheckError {
                                            message: format!("field '{}' expected {}, got {}", name, te_name(te), te_name(&actual)),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
                self.check_expr(object);
                self.check_expr(value);
            }
            Expr::Index { object, index } => { self.check_expr(object); self.check_expr(index); }
            Expr::IndexSet { object, index, value } => {
                self.check_expr(object); self.check_expr(index); self.check_expr(value);
            }
            Expr::Range { from, to } => { self.check_expr(from); self.check_expr(to); }
            Expr::ListLit(elems) => { for e in elems { self.check_expr(e); } }
            Expr::MapLit(pairs) => { for (_, v) in pairs { self.check_expr(v); } }
            Expr::Grouping(inner) => self.check_expr(inner),
            Expr::If { condition, then_branch, else_branch } => {
                self.check_expr(condition);
                self.push_scope();
                for s in then_branch { self.check_expr(s); }
                self.pop_scope();
                if let Some(branch) = else_branch {
                    self.push_scope();
                    for s in branch { self.check_expr(s); }
                    self.pop_scope();
                }
            }
            Expr::Begin {
                body,
                rescue_body,
                else_body,
                ..
            } => {
                for s in body { self.check_expr(s); }
                for s in rescue_body { self.check_expr(s); }
                for s in else_body { self.check_expr(s); }
            }
            Expr::Print(inner) => self.check_expr(inner),
            Expr::Class { methods, .. } => {
                for method in methods {
                    let saved = self.current_return_type.take();
                    self.current_return_type = method.return_type.clone();
                    self.push_scope();
                    for p in &method.params {
                        if let Some(te) = &p.type_ann { self.set_var(&p.name, te.clone()); }
                    }
                    for s in &method.body { self.check_expr(s); }
                    if let Some(rt) = &method.return_type.clone() {
                        if let Some(last_expr) = method.body.last() {
                            if let Some(actual) = self.infer_type(last_expr) {
                                if !types_compatible(&actual, rt) {
                                    self.errors.push(TypeCheckError {
                                        message: format!("return value expected {}, got {}", te_name(rt), te_name(&actual)),
                                    });
                                }
                            }
                        }
                    }
                    self.pop_scope();
                    self.current_return_type = saved;
                }
            }
            Expr::Function { name, params, return_type, body } => {
                self.functions.insert(name.clone(), FnSig {
                    params: params.clone(),
                    return_type: return_type.clone(),
                });
                let saved = self.current_return_type.take();
                self.current_return_type = return_type.clone();
                self.push_scope();
                for p in params {
                    if let Some(te) = &p.type_ann { self.set_var(&p.name, te.clone()); }
                }
                for s in body { self.check_expr(s); }
                if let Some(rt) = return_type {
                    if let Some(last_expr) = body.last() {
                        if let Some(actual) = self.infer_type(last_expr) {
                            if !types_compatible(&actual, rt) {
                                self.errors.push(TypeCheckError {
                                    message: format!("return value expected {}, got {}", te_name(rt), te_name(&actual)),
                                });
                            }
                        }
                    }
                }
                self.pop_scope();
                self.current_return_type = saved;
            }
            _ => {}
        }
    }

    fn check_call(&mut self, callee: &Expr, args: &[CallArg]) {
        for arg in args { self.check_expr(&arg.value); }

        match callee {
            Expr::Variable(name) => {
                if let Some(sig) = self.functions.get(name).cloned() {
                    self.check_args(&sig.params, args, name);
                }
            }
            Expr::Get { object, name: method_name } => {
                self.check_expr(object);
                if method_name == "new" {
                    if let Expr::Variable(class_name) = object.as_ref() {
                        if let Some(cls) = self.classes.get(class_name).cloned() {
                            for arg in args {
                                if let Some(fname) = &arg.name {
                                    if let Some(fd) = cls.fields.iter().find(|f| &f.name == fname) {
                                        if let Some(te) = &fd.type_ann {
                                            if let Some(actual) = self.infer_type(&arg.value) {
                                                if !types_compatible(&actual, te) {
                                                    self.errors.push(TypeCheckError {
                                                        message: format!("field '{}' expected {}, got {}", fname, te_name(te), te_name(&actual)),
                                                    });
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else if let Some(TypeExpr::Named(class_name)) = self.infer_type(object) {
                    if let Some(cls) = self.classes.get(&class_name).cloned() {
                        if let Some(sig) = cls.methods.get(method_name).cloned() {
                            self.check_args(&sig.params, args, method_name);
                        }
                    }
                }
            }
            _ => self.check_expr(callee),
        }
    }

    fn check_args(&mut self, params: &[ParamDef], args: &[CallArg], fn_name: &str) {
        for (param, arg) in params.iter().zip(args.iter()) {
            if let Some(te) = &param.type_ann {
                if let Some(actual) = self.infer_type(&arg.value) {
                    if !types_compatible(&actual, te) {
                        self.errors.push(TypeCheckError {
                            message: format!("argument '{}' to '{}' expected {}, got {}", param.name, fn_name, te_name(te), te_name(&actual)),
                        });
                    }
                }
            }
        }
    }

    fn infer_type(&self, expr: &Expr) -> Option<TypeExpr> {
        match expr {
            Expr::Literal(v) => match v {
                Value::Int(_)   => Some(TypeExpr::Named("Int".into())),
                Value::Float(_) => Some(TypeExpr::Named("Float".into())),
                Value::Str(_)   => Some(TypeExpr::Named("String".into())),
                Value::Bool(_)  => Some(TypeExpr::Named("Bool".into())),
                Value::Nil      => Some(TypeExpr::Named("Nil".into())),
                _ => None,
            },
            Expr::Variable(name) => self.get_var(name),
            Expr::Grouping(inner) => self.infer_type(inner),
            Expr::StringInterp(_) => Some(TypeExpr::Named("String".into())),
            Expr::ListLit(_) => Some(TypeExpr::Named("List".into())),
            Expr::MapLit(_)  => Some(TypeExpr::Named("Map".into())),
            Expr::Range { .. } => Some(TypeExpr::Named("Range".into())),
            Expr::Binary { left, op, right } => {
                match &op.kind {
                    TokenKind::Plus | TokenKind::Minus | TokenKind::Star |
                    TokenKind::Slash | TokenKind::Percent => {
                        let l = self.infer_type(left);
                        let r = self.infer_type(right);
                        match (&l, &r) {
                            (Some(TypeExpr::Named(a)), Some(TypeExpr::Named(b))) => {
                                if a == "Float" || b == "Float" { Some(TypeExpr::Named("Float".into())) }
                                else if a == "Int" && b == "Int" { Some(TypeExpr::Named("Int".into())) }
                                else { None }
                            }
                            _ => None,
                        }
                    }
                    TokenKind::EqEq | TokenKind::BangEq | TokenKind::Less | TokenKind::LessEq |
                    TokenKind::Greater | TokenKind::GreaterEq | TokenKind::AmpAmp | TokenKind::PipePipe => {
                        Some(TypeExpr::Named("Bool".into()))
                    }
                    _ => None,
                }
            }
            Expr::Print(inner) => self.infer_type(inner),
            Expr::If { .. }
            | Expr::Begin { .. }
            | Expr::While { .. }
            | Expr::MultiAssign { .. }
            | Expr::Return(_)
            | Expr::Break(_)
            | Expr::Next(_)
            | Expr::Raise(_) => None,
            Expr::Class { name, .. } => Some(TypeExpr::Named(name.clone())),
            Expr::Function { .. } => Some(TypeExpr::Named("String".into())),
            Expr::Call { callee, .. } => match callee.as_ref() {
                Expr::Variable(name) => {
                    self.functions.get(name).and_then(|s| s.return_type.clone())
                }
                Expr::Get { object, name: method_name } => {
                    if method_name == "new" {
                        if let Expr::Variable(cn) = object.as_ref() {
                            if self.classes.contains_key(cn) {
                                return Some(TypeExpr::Named(cn.clone()));
                            }
                        }
                    }
                    if let Some(TypeExpr::Named(cn)) = self.infer_type(object) {
                        if let Some(cls) = self.classes.get(&cn) {
                            return cls.methods.get(method_name).and_then(|s| s.return_type.clone());
                        }
                    }
                    None
                }
                _ => None,
            },
            _ => None,
        }
    }
}

fn types_compatible(actual: &TypeExpr, expected: &TypeExpr) -> bool {
    match (actual, expected) {
        (_, TypeExpr::Any) | (TypeExpr::Any, _) => true,
        (TypeExpr::Named(a), TypeExpr::Named(e)) => {
            a == e || (e == "Num" && (a == "Int" || a == "Float"))
        }
    }
}

fn te_name(te: &TypeExpr) -> &str {
    match te { TypeExpr::Named(n) => n.as_str(), TypeExpr::Any => "Any" }
}
