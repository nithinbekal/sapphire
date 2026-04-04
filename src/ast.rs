use crate::token::Token;

pub enum Expr {
    Literal(i64),
    Grouping(Box<Expr>),
    Binary {
        left: Box<Expr>,
        op: Token,
        right: Box<Expr>,
    },
}
