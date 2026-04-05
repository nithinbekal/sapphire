use crate::token::Token;
use crate::value::Value;

pub enum Stmt {
    Expression(Expr),
    Print(Expr),
}

pub enum Expr {
    Literal(Value),
    Grouping(Box<Expr>),
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
}
