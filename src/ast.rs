use crate::token::Token;

pub enum Stmt {
    Expression(Expr),
    Print(Expr),
}

pub enum Expr {
    Literal(i64),
    Grouping(Box<Expr>),
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
