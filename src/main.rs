mod ast;
mod interpreter;
mod lexer;
mod parser;
mod token;

fn main() {
    let source = "1 + 2 * 3";
    let tokens = lexer::Lexer::new(source).scan_tokens();
    let expr = parser::Parser::new(tokens).parse();
    let result = interpreter::evaluate(expr);
    println!("{} = {}", source, result);
}

