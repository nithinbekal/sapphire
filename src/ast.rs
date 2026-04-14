use crate::token::Token;
use crate::value::Value;

#[derive(Debug, Clone, PartialEq)]
pub enum TypeExpr {
    /// A bare type name: `Int`, `String`, `Bool`, `Float`, `Nil`, or a class name.
    Named(String),
    /// Escape hatch for future use (e.g. unannotated generics, explicit `Any` type).
    #[allow(dead_code)]
    Any,
}

#[derive(Debug, Clone)]
pub struct ParamDef {
    pub name: String,
    pub type_ann: Option<TypeExpr>,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub params: Vec<String>,
    pub body: Vec<Expr>,
}

#[derive(Debug, Clone)]
pub enum StringPart {
    Lit(String),
    Expr(Box<Expr>),
}

#[derive(Debug, Clone)]
pub struct FieldDef {
    pub name: String,
    pub type_ann: Option<TypeExpr>,
    pub default: Option<Expr>,
}

#[derive(Debug, Clone)]
pub struct MethodDef {
    pub name: String,
    pub params: Vec<ParamDef>,
    pub return_type: Option<TypeExpr>,
    pub body: Vec<Expr>,
    pub private: bool,
    pub class_method: bool,
}

#[derive(Debug, Clone)]
pub struct CallArg {
    pub name: Option<String>,
    pub value: Expr,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Literal(Value),
    Grouping(Box<Expr>),
    SelfExpr,
    Unary {
        op: Token,
        right: Box<Expr>,
    },
    Binary {
        left: Box<Expr>,
        op: Token,
        right: Box<Expr>,
    },
    Variable(String),
    Assign {
        name: String,
        value: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<CallArg>,
        block: Option<Block>,
    },
    Get {
        object: Box<Expr>,
        name: String,
    },
    SafeGet {
        object: Box<Expr>,
        name: String,
    },
    Set {
        object: Box<Expr>,
        name: String,
        value: Box<Expr>,
    },
    StringInterp(Vec<StringPart>),
    ListLit(Vec<Expr>),
    MapLit(Vec<(String, Expr)>),
    Super {
        method: String,
        args: Vec<CallArg>,
        block: Option<Block>,
    },
    Index {
        object: Box<Expr>,
        index: Box<Expr>,
    },
    IndexSet {
        object: Box<Expr>,
        index: Box<Expr>,
        value: Box<Expr>,
    },
    Yield {
        args: Vec<CallArg>,
    },
    Range {
        from: Box<Expr>,
        to: Box<Expr>,
    },
    /// `if` / `elsif` / `else` — value is the last expression in the taken branch, or `nil`.
    If {
        condition: Box<Expr>,
        then_branch: Vec<Expr>,
        else_branch: Option<Vec<Expr>>,
    },
    /// `print expr` — evaluates `expr`, prints, value is the printed value.
    Print(Box<Expr>),
    /// `class Name ...` — defines a class; value is the class object.
    Class {
        name: String,
        /// Superclass expression: `Name` or `Outer.Inner`.  `None` means inherit from Object.
        superclass: Option<Box<Expr>>,
        fields: Vec<FieldDef>,
        methods: Vec<MethodDef>,
        /// Nested class definitions — accessible only as `Outer.Inner`.
        nested: Vec<Expr>,
    },
    /// Top-level `def name(...)` on `Object`; value is the method name string.
    Function {
        name: String,
        params: Vec<ParamDef>,
        return_type: Option<TypeExpr>,
        body: Vec<Expr>,
    },
    /// `begin … rescue … else … end` — value follows Ruby (last expression on taken path).
    Begin {
        body: Vec<Expr>,
        rescue_var: Option<String>,
        rescue_body: Vec<Expr>,
        else_body: Vec<Expr>,
    },
    /// `while cond { ... }` — value is `nil` when the loop exits normally (Ruby).
    While {
        condition: Box<Expr>,
        body: Vec<Expr>,
    },
    Return(Box<Expr>),
    Break(Box<Expr>),
    Next(Box<Expr>),
    Raise(Box<Expr>),
    MultiAssign {
        names: Vec<String>,
        values: Vec<Expr>,
    },
    /// Anonymous `def(params) { body }` — a first-class lambda value.
    Lambda {
        params: Vec<String>,
        body: Vec<Expr>,
    },
    /// `import "./path"` — load and execute a relative file in the current scope.
    Import { path: String },
}
