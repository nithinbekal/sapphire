use crate::ast::{CallArg, Expr, FieldDef, ParamDef, TypeExpr};
use crate::token::TokenKind;
use crate::value::Value;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct TypeCheckError {
    pub message: String,
}

impl std::fmt::Display for TypeCheckError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "type error: {}", self.message)
    }
}

/// Result of a full typecheck, including any errors and the resolved (annotated or inferred) return
/// types of top-level functions and class methods.
pub struct TypeCheckInfo {
    pub errors: Vec<TypeCheckError>,
    pub types: CheckedTypes,
}

/// Return types of functions and methods as recorded by the typechecker after a successful pass.
/// Look up with [`CheckedTypes::function_return_type`] and [`CheckedTypes::method_return_type`].
/// Each lookup returns `None` if the name is not in the program; inner [`Option`] is `None` if no
/// return type is known.
#[derive(Debug, Clone)]
pub struct CheckedTypes {
    function_returns: HashMap<String, Option<TypeExpr>>,
    class_method_returns: HashMap<String, HashMap<String, Option<TypeExpr>>>,
}

impl CheckedTypes {
    /// Outer `None`: no such top-level function. Inner: inferred or annotated return type, if any.
    pub fn function_return_type(&self, name: &str) -> Option<Option<TypeExpr>> {
        self.function_returns.get(name).cloned()
    }

    /// Outer `None`: class or method missing from the program. Inner: return type, if any.
    pub fn method_return_type(&self, class: &str, method: &str) -> Option<Option<TypeExpr>> {
        self.class_method_returns
            .get(class)
            .and_then(|m| m.get(method))
            .cloned()
    }
}

#[derive(Clone)]
struct FnSig {
    #[allow(dead_code)]
    type_params: Vec<String>,
    params: Vec<ParamDef>,
    return_type: Option<TypeExpr>,
}

#[derive(Clone)]
struct ClassInfo {
    #[allow(dead_code)]
    type_params: Vec<String>,
    fields: Vec<FieldDef>,
    methods: HashMap<String, FnSig>,
}

pub struct TypeChecker {
    functions: HashMap<String, FnSig>,
    classes: HashMap<String, ClassInfo>,
    type_aliases: HashMap<String, TypeExpr>,
    errors: Vec<TypeCheckError>,
    current_return_type: Option<TypeExpr>,
    /// Name of the class whose methods are currently being checked, enabling `SelfExpr` inference.
    current_class: Option<String>,
    var_scopes: Vec<HashMap<String, TypeExpr>>,
    /// Stacked scopes of in-scope type variable names (from generic class/function params).
    type_vars: Vec<HashSet<String>>,
}

impl TypeChecker {
    fn new() -> Self {
        Self {
            functions: HashMap::new(),
            classes: HashMap::new(),
            type_aliases: HashMap::new(),
            errors: Vec::new(),
            current_return_type: None,
            current_class: None,
            var_scopes: vec![HashMap::new()],
            type_vars: Vec::new(),
        }
    }

    pub fn check(exprs: &[Expr]) -> Vec<TypeCheckError> {
        Self::check_info(exprs).errors
    }

    /// Like [`TypeChecker::check`], but also returns resolved function and method return types.
    pub fn check_info(exprs: &[Expr]) -> TypeCheckInfo {
        let mut tc = Self::new();
        for e in exprs {
            tc.collect_def(e);
        }
        for e in exprs {
            tc.check_expr(e);
        }
        loop {
            let progress = exprs.iter().any(|e| tc.propagate_return_type(e));
            if !progress {
                break;
            }
        }
        let errors = std::mem::take(&mut tc.errors);
        let types = tc.into_checked_types();
        TypeCheckInfo { errors, types }
    }

    fn into_checked_types(self) -> CheckedTypes {
        let function_returns = self
            .functions
            .into_iter()
            .map(|(name, sig)| (name, sig.return_type))
            .collect();
        let class_method_returns = self
            .classes
            .into_iter()
            .map(|(name, class)| {
                let methods: HashMap<String, Option<TypeExpr>> = class
                    .methods
                    .into_iter()
                    .map(|(mname, sig)| (mname, sig.return_type))
                    .collect();
                (name, methods)
            })
            .collect();
        CheckedTypes {
            function_returns,
            class_method_returns,
        }
    }

    /// Resolve a type expression by expanding any named aliases.
    fn resolve_type(&self, te: TypeExpr) -> TypeExpr {
        match te {
            TypeExpr::Named(ref n) => {
                if let Some(expanded) = self.type_aliases.get(n) {
                    self.resolve_type(expanded.clone())
                } else {
                    te
                }
            }
            TypeExpr::Apply(name, args) => {
                TypeExpr::Apply(name, args.into_iter().map(|a| self.resolve_type(a)).collect())
            }
            TypeExpr::Union(arms) => {
                let resolved: Vec<TypeExpr> = arms.into_iter().map(|a| self.resolve_type(a)).collect();
                // Flatten nested unions that arose from alias expansion
                let mut flat = Vec::new();
                for arm in resolved {
                    match arm {
                        TypeExpr::Union(inner) => flat.extend(inner),
                        other => flat.push(other),
                    }
                }
                if flat.len() == 1 {
                    flat.remove(0)
                } else {
                    TypeExpr::Union(flat)
                }
            }
            TypeExpr::Literal(_) => te,
            TypeExpr::Any => TypeExpr::Any,
        }
    }

    fn push_type_vars(&mut self, params: &[String]) {
        self.type_vars.push(params.iter().cloned().collect());
    }

    fn pop_type_vars(&mut self) {
        self.type_vars.pop();
    }

    fn is_type_var(&self, name: &str) -> bool {
        self.type_vars.iter().rev().any(|scope| scope.contains(name))
    }

    fn types_compat(&self, actual: &TypeExpr, expected: &TypeExpr) -> bool {
        // A type variable is compatible with anything (acts like Any within its scope).
        if let TypeExpr::Named(n) = expected
            && self.is_type_var(n)
        {
            return true;
        }
        if let TypeExpr::Named(n) = actual
            && self.is_type_var(n)
        {
            return true;
        }
        let a = self.resolve_type(actual.clone());
        let e = self.resolve_type(expected.clone());
        types_compatible(&a, &e)
    }

    // First pass: record function and class signatures without checking bodies.
    fn collect_def(&mut self, expr: &Expr) {
        match expr {
            Expr::TypeAlias { name, type_expr } => {
                self.type_aliases.insert(name.clone(), type_expr.clone());
            }
            Expr::Function {
                name,
                type_params,
                params,
                return_type,
                ..
            } => {
                self.functions.insert(
                    name.clone(),
                    FnSig {
                        type_params: type_params.clone(),
                        params: params.clone(),
                        return_type: return_type.clone(),
                    },
                );
            }
            Expr::Class {
                name,
                type_params,
                fields,
                methods,
                ..
            } => {
                let method_sigs = methods
                    .iter()
                    .map(|m| {
                        (
                            m.name.clone(),
                            FnSig {
                                type_params: m.type_params.clone(),
                                params: m.params.clone(),
                                return_type: m.return_type.clone(),
                            },
                        )
                    })
                    .collect();
                self.classes.insert(
                    name.clone(),
                    ClassInfo {
                        type_params: type_params.clone(),
                        fields: fields.clone(),
                        methods: method_sigs,
                    },
                );
            }
            _ => {}
        }
    }

    fn push_scope(&mut self) {
        self.var_scopes.push(HashMap::new());
    }
    fn pop_scope(&mut self) {
        self.var_scopes.pop();
    }

    fn validate_type_ann(&mut self, te: &TypeExpr) {
        match te {
            TypeExpr::Apply(_, args) => {
                for arg in args {
                    self.validate_type_ann(arg);
                }
            }
            _ => {
                let resolved = self.resolve_type(te.clone());
                if let Some(msg) = check_union_duplicates(&resolved) {
                    self.errors.push(TypeCheckError { message: msg });
                }
            }
        }
    }

    fn set_var(&mut self, name: &str, ty: TypeExpr) {
        if let Some(scope) = self.var_scopes.last_mut() {
            scope.insert(name.to_string(), ty);
        }
    }

    fn get_var(&self, name: &str) -> Option<TypeExpr> {
        for scope in self.var_scopes.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return Some(ty.clone());
            }
        }
        None
    }

    fn check_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Return(inner) => {
                if let Some(rt) = self.current_return_type.clone()
                    && let Some(actual) = self.infer_type(inner)
                    && !self.types_compat(&actual, &rt)
                {
                    self.errors.push(TypeCheckError {
                        message: format!(
                            "return value expected {}, got {}",
                            te_name(&rt),
                            te_name(&actual)
                        ),
                    });
                }
                self.check_expr(inner);
            }
            Expr::While { condition, body } => {
                self.check_expr(condition);
                self.push_scope();
                for s in body {
                    self.check_expr(s);
                }
                self.pop_scope();
            }
            Expr::Lambda { body, .. } => {
                self.push_scope();
                for s in body {
                    self.check_expr(s);
                }
                self.pop_scope();
            }
            Expr::Raise(inner) => self.check_expr(inner),
            Expr::Break(inner) | Expr::Next(inner) => self.check_expr(inner),
            Expr::MultiAssign { names, values } => {
                for (name, ve) in names.iter().zip(values.iter()) {
                    if let Some(ty) = self.infer_type(ve) {
                        self.set_var(name, ty);
                    }
                    self.check_expr(ve);
                }
            }
            Expr::Call { callee, args, .. } => self.check_call(callee, args),
            Expr::Assign { name, value } => {
                let ty = self.infer_type(value).or_else(|| {
                    if let Expr::Call { callee, .. } = value.as_ref()
                        && let Expr::Get { object, name: m } = callee.as_ref()
                        && m == "new"
                        && let Expr::Variable(cn) = object.as_ref()
                        && self.classes.contains_key(cn)
                    {
                        return Some(TypeExpr::Named(cn.clone()));
                    }
                    None
                });
                if let Some(ty) = ty {
                    self.set_var(name, ty);
                }
                self.check_expr(value);
            }
            Expr::Binary { left, right, .. } => {
                self.check_expr(left);
                self.check_expr(right);
            }
            Expr::Unary { right, .. } => self.check_expr(right),
            Expr::Get { object, .. } | Expr::SafeGet { object, .. } => self.check_expr(object),
            Expr::Set {
                object,
                value,
                name,
            } => {
                // If we can determine the receiver's class, check the field type.
                if let Some(TypeExpr::Named(class_name)) = self.infer_type(object)
                    && let Some(cls) = self.classes.get(&class_name).cloned()
                    && let Some(fd) = cls.fields.iter().find(|f| &f.name == name)
                    && let Some(te) = &fd.type_ann
                    && let Some(actual) = self.infer_type(value)
                    && !self.types_compat(&actual, te)
                {
                    self.errors.push(TypeCheckError {
                        message: format!(
                            "field '{}' expected {}, got {}",
                            name,
                            te_name(te),
                            te_name(&actual)
                        ),
                    });
                }
                self.check_expr(object);
                self.check_expr(value);
            }
            Expr::Index { object, index } => {
                self.check_expr(object);
                self.check_expr(index);
            }
            Expr::IndexSet {
                object,
                index,
                value,
            } => {
                self.check_expr(object);
                self.check_expr(index);
                self.check_expr(value);
            }
            Expr::Range { from, to } => {
                self.check_expr(from);
                self.check_expr(to);
            }
            Expr::ListLit(elems) => {
                for e in elems {
                    self.check_expr(e);
                }
            }
            Expr::MapLit(pairs) => {
                for (_, v) in pairs {
                    self.check_expr(v);
                }
            }
            Expr::Grouping(inner) => self.check_expr(inner),
            Expr::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.check_expr(condition);
                self.push_scope();
                for s in then_branch {
                    self.check_expr(s);
                }
                self.pop_scope();
                if let Some(branch) = else_branch {
                    self.push_scope();
                    for s in branch {
                        self.check_expr(s);
                    }
                    self.pop_scope();
                }
            }
            Expr::Begin {
                body,
                rescue_body,
                else_body,
                ..
            } => {
                for s in body {
                    self.check_expr(s);
                }
                for s in rescue_body {
                    self.check_expr(s);
                }
                for s in else_body {
                    self.check_expr(s);
                }
            }
            Expr::Print(inner) => self.check_expr(inner),
            Expr::Class { name, type_params, methods, .. } => {
                let saved_class = self.current_class.replace(name.clone());
                self.push_type_vars(type_params);
                for method in methods {
                    let saved = self.current_return_type.take();
                    if let Some(rt) = &method.return_type {
                        self.validate_type_ann(rt);
                    }
                    self.current_return_type = method.return_type.clone();
                    self.push_scope();
                    self.push_type_vars(&method.type_params);

                    for p in &method.params {
                        if let Some(te) = &p.type_ann {
                            self.validate_type_ann(te);
                            self.set_var(&p.name, te.clone());
                        }
                    }

                    for s in &method.body {
                        self.check_expr(s);
                    }

                    if let Some(rt) = &method.return_type.clone()
                        && let Some(last_expr) = method.body.last()
                        && let Some(actual) = self.infer_type(last_expr)
                        && !self.types_compat(&actual, rt)
                    {
                        self.errors.push(TypeCheckError {
                            message: format!(
                                "return value expected {}, got {}",
                                te_name(rt),
                                te_name(&actual)
                            ),
                        });
                    }

                    if method.return_type.is_none()
                        && let Some(last) = method.body.last()
                        && let Some(inferred) = self.infer_type(last)
                        && let Some(cls) = self.classes.get_mut(name)
                        && let Some(sig) = cls.methods.get_mut(&method.name)
                    {
                        sig.return_type = Some(inferred);
                    }

                    self.pop_type_vars();
                    self.pop_scope();
                    self.current_return_type = saved;
                }
                self.pop_type_vars();
                self.current_class = saved_class;
            }
            Expr::Function {
                name,
                type_params,
                params,
                return_type,
                body,
            } => {
                self.functions.insert(
                    name.clone(),
                    FnSig {
                        type_params: type_params.clone(),
                        params: params.clone(),
                        return_type: return_type.clone(),
                    },
                );
                let saved = self.current_return_type.take();
                if let Some(rt) = return_type {
                    self.validate_type_ann(rt);
                }
                self.current_return_type = return_type.clone();
                self.push_scope();
                self.push_type_vars(type_params);
                for p in params {
                    if let Some(te) = &p.type_ann {
                        self.validate_type_ann(te);
                        self.set_var(&p.name, te.clone());
                    }
                }
                for s in body {
                    self.check_expr(s);
                }
                if let Some(rt) = return_type
                    && let Some(last_expr) = body.last()
                    && let Some(actual) = self.infer_type(last_expr)
                    && !self.types_compat(&actual, rt)
                {
                    self.errors.push(TypeCheckError {
                        message: format!(
                            "return value expected {}, got {}",
                            te_name(rt),
                            te_name(&actual)
                        ),
                    });
                }

                if return_type.is_none()
                    && let Some(last) = body.last()
                    && let Some(inferred) = self.infer_type(last)
                    && let Some(sig) = self.functions.get_mut(name)
                {
                    sig.return_type = Some(inferred);
                }

                self.pop_type_vars();
                self.pop_scope();
                self.current_return_type = saved;
            }
            Expr::TypeAlias { .. } => {}
            _ => {}
        }
    }

    /// Re-infer the return type for any unannotated function/method that still has `None` because
    /// its callee was defined later in the source and not yet inferred during `check_expr`.
    /// Returns `true` if at least one type was newly stored.
    fn propagate_return_type(&mut self, expr: &Expr) -> bool {
        match expr {
            Expr::Function { name, type_params, params, return_type, body } => {
                if return_type.is_some() {
                    return false;
                }
                if self.functions.get(name).and_then(|s| s.return_type.as_ref()).is_some() {
                    return false;
                }
                self.push_type_vars(type_params);
                self.push_scope();
                for p in params {
                    if let Some(te) = &p.type_ann {
                        self.set_var(&p.name, te.clone());
                    }
                }
                let inferred = body.last().and_then(|e| self.infer_type(e));
                self.pop_scope();
                self.pop_type_vars();
                if let Some(ty) = inferred
                    && let Some(sig) = self.functions.get_mut(name)
                {
                    sig.return_type = Some(ty);
                    return true;
                }
                false
            }
            Expr::Class { name, type_params, methods, .. } => {
                let mut progress = false;
                let saved_class = self.current_class.replace(name.clone());
                self.push_type_vars(type_params);
                for method in methods {
                    if method.return_type.is_some() {
                        continue;
                    }
                    let already_known = self
                        .classes
                        .get(name)
                        .and_then(|c| c.methods.get(&method.name))
                        .and_then(|s| s.return_type.as_ref())
                        .is_some();
                    if already_known {
                        continue;
                    }
                    self.push_type_vars(&method.type_params);
                    self.push_scope();
                    for p in &method.params {
                        if let Some(te) = &p.type_ann {
                            self.set_var(&p.name, te.clone());
                        }
                    }
                    let inferred = method.body.last().and_then(|e| self.infer_type(e));
                    self.pop_scope();
                    self.pop_type_vars();
                    if let Some(ty) = inferred
                        && let Some(cls) = self.classes.get_mut(name)
                        && let Some(sig) = cls.methods.get_mut(&method.name)
                    {
                        sig.return_type = Some(ty);
                        progress = true;
                    }
                }
                self.pop_type_vars();
                self.current_class = saved_class;
                progress
            }
            _ => false,
        }
    }

    fn check_call(&mut self, callee: &Expr, args: &[CallArg]) {
        for arg in args {
            self.check_expr(&arg.value);
        }

        match callee {
            Expr::Variable(name) => {
                if let Some(sig) = self.functions.get(name).cloned() {
                    // Push the function's type params so they are treated as Any at call sites.
                    self.push_type_vars(&sig.type_params.clone());
                    self.check_args(&sig.params, args, name);
                    self.pop_type_vars();
                }
            }
            Expr::Get {
                object,
                name: method_name,
            } => {
                self.check_expr(object);
                if method_name == "new" {
                    if let Expr::Variable(class_name) = object.as_ref()
                        && let Some(cls) = self.classes.get(class_name).cloned()
                    {
                        // Push the class's type params so fields typed as T accept any value.
                        self.push_type_vars(&cls.type_params.clone());
                        for arg in args {
                            if let Some(fname) = &arg.name
                                && let Some(fd) = cls.fields.iter().find(|f| &f.name == fname)
                                && let Some(te) = &fd.type_ann
                                && let Some(actual) = self.infer_type(&arg.value)
                                && !self.types_compat(&actual, te)
                            {
                                self.errors.push(TypeCheckError {
                                    message: format!(
                                        "field '{}' expected {}, got {}",
                                        fname,
                                        te_name(te),
                                        te_name(&actual)
                                    ),
                                });
                            }
                        }
                        self.pop_type_vars();
                    }
                } else if let Some(TypeExpr::Named(class_name)) = self.infer_type(object)
                    && let Some(cls) = self.classes.get(&class_name).cloned()
                    && let Some(sig) = cls.methods.get(method_name).cloned()
                {
                    // Push class + method type params so T params accept any argument.
                    let combined: Vec<String> = cls.type_params.iter()
                        .chain(sig.type_params.iter())
                        .cloned()
                        .collect();
                    self.push_type_vars(&combined);
                    self.check_args(&sig.params, args, method_name);
                    self.pop_type_vars();
                }
            }
            _ => self.check_expr(callee),
        }
    }

    fn check_args(&mut self, params: &[ParamDef], args: &[CallArg], fn_name: &str) {
        for (param, arg) in params.iter().zip(args.iter()) {
            if let Some(te) = &param.type_ann
                && let Some(actual) = self.infer_type(&arg.value)
                && !self.types_compat(&actual, te)
            {
                self.errors.push(TypeCheckError {
                    message: format!(
                        "argument '{}' to '{}' expected {}, got {}",
                        param.name,
                        fn_name,
                        te_name(te),
                        te_name(&actual)
                    ),
                });
            }
        }
    }

    fn infer_type(&self, expr: &Expr) -> Option<TypeExpr> {
        match expr {
            Expr::SelfExpr => self.current_class.as_ref().map(|cn| TypeExpr::Named(cn.clone())),
            Expr::Literal(v) => match v {
                Value::Int(_) => Some(TypeExpr::Named("Int".into())),
                Value::Float(_) => Some(TypeExpr::Named("Float".into())),
                Value::Str(_) => Some(TypeExpr::Named("String".into())),
                Value::Bool(_) => Some(TypeExpr::Named("Bool".into())),
                Value::Nil => Some(TypeExpr::Named("Nil".into())),
            },
            Expr::Variable(name) => self.get_var(name),
            Expr::Grouping(inner) => self.infer_type(inner),
            Expr::StringInterp(_) => Some(TypeExpr::Named("String".into())),
            Expr::ListLit(_) => Some(TypeExpr::Named("List".into())),
            Expr::MapLit(_) => Some(TypeExpr::Named("Map".into())),
            Expr::Range { .. } => Some(TypeExpr::Named("Range".into())),
            Expr::Binary { left, op, right } => match &op.kind {
                TokenKind::Plus => {
                    let l = self.infer_type(left);
                    let r = self.infer_type(right);
                    match (&l, &r) {
                        (Some(TypeExpr::Named(a)), Some(TypeExpr::Named(b))) => {
                            if a == "String" && b == "String" {
                                Some(TypeExpr::Named("String".into()))
                            } else if a == "Float" || b == "Float" {
                                Some(TypeExpr::Named("Float".into()))
                            } else if a == "Int" && b == "Int" {
                                Some(TypeExpr::Named("Int".into()))
                            } else {
                                None
                            }
                        }
                        _ => None,
                    }
                }
                TokenKind::Minus
                | TokenKind::Star
                | TokenKind::Slash
                | TokenKind::Percent => {
                    let l = self.infer_type(left);
                    let r = self.infer_type(right);
                    match (&l, &r) {
                        (Some(TypeExpr::Named(a)), Some(TypeExpr::Named(b))) => {
                            if a == "Float" || b == "Float" {
                                Some(TypeExpr::Named("Float".into()))
                            } else if a == "Int" && b == "Int" {
                                Some(TypeExpr::Named("Int".into()))
                            } else {
                                None
                            }
                        }
                        _ => None,
                    }
                }
                TokenKind::EqEq
                | TokenKind::BangEq
                | TokenKind::Less
                | TokenKind::LessEq
                | TokenKind::Greater
                | TokenKind::GreaterEq
                | TokenKind::AmpAmp
                | TokenKind::PipePipe => Some(TypeExpr::Named("Bool".into())),
                _ => None,
            },
            Expr::Unary { op, right } => match &op.kind {
                TokenKind::Bang => Some(TypeExpr::Named("Bool".into())),
                TokenKind::Tilde => Some(TypeExpr::Named("Int".into())),
                TokenKind::Minus => {
                    if let Some(TypeExpr::Named(n)) = self.infer_type(right)
                        && (n == "Int" || n == "Float")
                    {
                        return Some(TypeExpr::Named(n));
                    }
                    None
                }
                _ => None,
            },
            Expr::Print(inner) => self.infer_type(inner),
            Expr::If { then_branch, else_branch, .. } => {
                let then_type = then_branch.last().and_then(|e| self.infer_type(e))?;
                let else_stmts = else_branch.as_ref()?;
                let else_type = else_stmts.last().and_then(|e| self.infer_type(e))?;
                if then_type == else_type { Some(then_type) } else { None }
            }
            Expr::Begin { body, rescue_body, .. } => {
                if rescue_body.is_empty() {
                    body.last().and_then(|e| self.infer_type(e))
                } else {
                    None
                }
            }
            Expr::Return(inner) => self.infer_type(inner),
            Expr::While { .. } => Some(TypeExpr::Named("Nil".into())),
            Expr::MultiAssign { .. }
            | Expr::Break(_)
            | Expr::Next(_)
            | Expr::Raise(_) => None,
            Expr::Lambda { .. } => None,
            Expr::Class { name, .. } => Some(TypeExpr::Named(name.clone())),
            Expr::Function { .. } => Some(TypeExpr::Named("String".into())),
            Expr::Call { callee, .. } => match callee.as_ref() {
                Expr::Variable(name) => {
                    self.functions.get(name).and_then(|s| s.return_type.clone())
                }
                Expr::Get {
                    object,
                    name: method_name,
                } => {
                    if method_name == "new"
                        && let Expr::Variable(cn) = object.as_ref()
                        && self.classes.contains_key(cn)
                    {
                        return Some(TypeExpr::Named(cn.clone()));
                    }

                    if let Some(TypeExpr::Named(cn)) = self.infer_type(object)
                        && let Some(cls) = self.classes.get(&cn)
                    {
                        return cls
                            .methods
                            .get(method_name)
                            .and_then(|s| s.return_type.clone());
                    }

                    None
                }
                Expr::SafeGet {
                    object,
                    name: method_name,
                } => {
                    if let Some(TypeExpr::Named(cn)) = self.infer_type(object)
                        && let Some(cls) = self.classes.get(&cn)
                        && let Some(ret) = cls
                            .methods
                            .get(method_name)
                            .and_then(|s| s.return_type.clone())
                    {
                        return Some(TypeExpr::Union(vec![
                            TypeExpr::Named("Nil".into()),
                            ret,
                        ]));
                    }
                    None
                }
                _ => None,
            },
            Expr::Assign { value, .. } => self.infer_type(value),
            Expr::Set { value, .. } => self.infer_type(value),
            Expr::Index { object, .. } => {
                // List literal with uniform element type → element type.
                if let Expr::ListLit(elems) = object.as_ref() {
                    let mut elem_type = None;
                    for e in elems {
                        let t = self.infer_type(e);
                        if elem_type.is_none() {
                            elem_type = t;
                        } else if t != elem_type {
                            return None;
                        }
                    }
                    return elem_type;
                }
                // Map literal with uniform value type → value type.
                if let Expr::MapLit(pairs) = object.as_ref() {
                    let mut val_type = None;
                    for (_, v) in pairs {
                        let t = self.infer_type(v);
                        if val_type.is_none() {
                            val_type = t;
                        } else if t != val_type {
                            return None;
                        }
                    }
                    return val_type;
                }
                // Parameterized types: List[T] → T, Map[K,V] → V.
                if let Some(ty) = self.infer_type(object) {
                    match ty {
                        TypeExpr::Apply(name, args) if name == "List" && args.len() == 1 => {
                            return Some(args[0].clone());
                        }
                        TypeExpr::Apply(name, args) if name == "Map" && args.len() == 2 => {
                            return Some(args[1].clone());
                        }
                        _ => {}
                    }
                }
                None
            }
            _ => None,
        }
    }
}

fn types_compatible(actual: &TypeExpr, expected: &TypeExpr) -> bool {
    let literal_base_named = |v: &Value| match v {
        Value::Int(_) => TypeExpr::Named("Int".to_string()),
        Value::Float(_) => TypeExpr::Named("Float".to_string()),
        Value::Str(_) => TypeExpr::Named("String".to_string()),
        Value::Bool(_) => TypeExpr::Named("Bool".to_string()),
        Value::Nil => TypeExpr::Named("Nil".to_string()),
    };

    match (actual, expected) {
        (_, TypeExpr::Any) | (TypeExpr::Any, _) => true,
        // Structural Apply matching: name and all type args must match.
        (TypeExpr::Apply(an, a_args), TypeExpr::Apply(en, e_args)) => {
            an == en
                && a_args.len() == e_args.len()
                && a_args.iter().zip(e_args.iter()).all(|(a, e)| types_compatible(a, e))
        }
        // Gradual: bare Named is compatible with Apply of the same name (unannotated = unknown params).
        (TypeExpr::Named(a), TypeExpr::Apply(e, _)) | (TypeExpr::Apply(a, _), TypeExpr::Named(e)) => a == e,
        // actual is a union: ALL arms must be compatible with expected
        (TypeExpr::Union(arms), _) => arms.iter().all(|a| types_compatible(a, expected)),
        // expected is a union: actual must be compatible with AT LEAST ONE arm
        (_, TypeExpr::Union(arms)) => arms.iter().any(|e| types_compatible(actual, e)),
        (TypeExpr::Literal(a), TypeExpr::Literal(e)) => a == e,
        (TypeExpr::Literal(a), TypeExpr::Named(e)) => {
            let base = literal_base_named(a);
            types_compatible(&base, &TypeExpr::Named(e.clone()))
        }
        (TypeExpr::Named(a), TypeExpr::Named(e)) => {
            a == e || (e == "Num" && (a == "Int" || a == "Float"))
        }
        (TypeExpr::Named(_), TypeExpr::Literal(_))
        | (TypeExpr::Apply(_, _), TypeExpr::Literal(_))
        | (TypeExpr::Literal(_), TypeExpr::Apply(_, _)) => false,
    }
}

fn te_name(te: &TypeExpr) -> String {
    match te {
        TypeExpr::Named(n) => n.clone(),
        TypeExpr::Apply(n, args) => {
            format!("{}[{}]", n, args.iter().map(te_name).collect::<Vec<_>>().join(", "))
        }
        TypeExpr::Literal(Value::Int(n)) => n.to_string(),
        TypeExpr::Literal(Value::Float(n)) => n.to_string(),
        TypeExpr::Literal(Value::Str(s)) => format!("{:?}", s),
        TypeExpr::Literal(Value::Bool(b)) => b.to_string(),
        TypeExpr::Literal(Value::Nil) => "Nil".to_string(),
        TypeExpr::Any => "Any".to_string(),
        TypeExpr::Union(arms) => arms.iter().map(te_name).collect::<Vec<_>>().join(" | "),
    }
}

fn check_union_duplicates(te: &TypeExpr) -> Option<String> {
    if let TypeExpr::Union(arms) = te {
        let mut seen = std::collections::HashSet::new();
        for arm in arms {
            let key = te_name(arm);
            if !seen.insert(key.clone()) {
                return Some(format!("duplicate type '{}' in union", key));
            }
        }
    }
    None
}
