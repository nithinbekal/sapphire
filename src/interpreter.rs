use crate::ast::Expr;
use crate::token::TokenKind;

pub fn evaluate(expr: Expr) -> i64 {
    match expr {
        Expr::Literal(n) => n,
        Expr::Grouping(inner) => evaluate(*inner),
        Expr::Binary { left, op, right } => {
            let l = evaluate(*left);
            let r = evaluate(*right);
            match op.kind {
                TokenKind::Plus  => l + r,
                TokenKind::Minus => l - r,
                TokenKind::Star  => l * r,
                TokenKind::Slash => l / r,
                _ => panic!("unknown binary operator: {:?}", op.kind),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn run(source: &str) -> i64 {
        let tokens = Lexer::new(source).scan_tokens();
        let expr = Parser::new(tokens).parse();
        evaluate(expr)
    }

    #[test]
    fn test_literal() {
        assert_eq!(run("42"), 42);
    }

    #[test]
    fn test_addition() {
        assert_eq!(run("1+2"), 3);
    }

    #[test]
    fn test_precedence() {
        assert_eq!(run("1+2*3"), 7);
    }

    #[test]
    fn test_grouping() {
        assert_eq!(run("(1+2)*3"), 9);
    }

    #[test]
    fn test_subtraction() {
        assert_eq!(run("10-3-2"), 5);
    }

    #[test]
    fn test_division() {
        assert_eq!(run("10/2"), 5);
    }
}
