mod ast;
mod environment;
mod error;
mod interpreter;
mod lexer;
mod parser;
mod token;
mod value;

use std::io::{self, Write};

fn main() {
    println!("Sapphire 0.1.0 — :q to quit");

    let env = environment::Environment::new();

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
        match parser::Parser::new(tokens).parse() {
            Err(e) => eprintln!("{}", e),
            Ok(stmts) => {
                for stmt in stmts {
                    match interpreter::execute(stmt, env.clone()) {
                        Ok(Some(result)) if result != value::Value::Nil => println!("{}", result),
                        Ok(_) => {}
                        Err(e) => { eprintln!("{}", e); break; }
                    }
                }
            }
        }
    }
}

