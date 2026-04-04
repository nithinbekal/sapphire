mod ast;
mod interpreter;
mod lexer;
mod parser;
mod token;

use std::io::{self, Write};

fn main() {
    println!("Sapphire 0.1.0 — :q to quit");

    loop {
        print!("> ");
        io::stdout().flush().unwrap();

        let mut line = String::new();
        if io::stdin().read_line(&mut line).is_err() || line.trim() == ":q" {
            break;
        }

        let source = line.trim();
        if source.is_empty() {
            continue;
        }

        let tokens = lexer::Lexer::new(source).scan_tokens();
        let expr = parser::Parser::new(tokens).parse();
        let result = interpreter::evaluate(expr);
        println!("{}", result);
    }
}

