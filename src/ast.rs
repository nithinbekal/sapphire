use crate::token::Token;
use crate::value::Value;

#[derive(Debug, Clone)]
pub struct Block {
    pub params: Vec<String>,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub enum StringPart {
    Lit(String),
    Expr(Box<Expr>),
}

#[derive(Debug, Clone)]
pub struct FieldDef {
    pub name: String,
    pub type_name: Option<String>,
    pub default: Option<Expr>,
}

#[derive(Debug, Clone)]
pub struct MethodDef {
    pub name: String,
    pub params: Vec<String>,
    pub body: Vec<Stmt>,
    pub private: bool,
}

#[derive(Debug, Clone)]
pub struct CallArg {
    pub name: Option<String>,
    pub value: Expr,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Expression(Expr),
    Print(Expr),
    If {
        condition: Expr,
        then_branch: Vec<Stmt>,
        else_branch: Option<Vec<Stmt>>,
    },
    While {
        condition: Expr,
        body: Vec<Stmt>,
    },
    Function {
        name: String,
        params: Vec<String>,
        body: Vec<Stmt>,
    },
    Return(Expr),
    Break(Expr),
    Next(Expr),
    Class {
        name: String,
        superclass: Option<String>,
        fields: Vec<FieldDef>,
        methods: Vec<MethodDef>,
    },
    Raise(Expr),
    MultiAssign {
        names: Vec<String>,
        values: Vec<Expr>,
    },
    Begin {
        body: Vec<Stmt>,
        rescue_var: Option<String>,
        rescue_body: Vec<Stmt>,
        else_body: Vec<Stmt>,
    },
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
}
