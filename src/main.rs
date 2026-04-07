mod ast;
mod environment;
mod error;
mod interpreter;
mod lexer;
mod parser;
mod token;
mod typechecker;
mod value;

use std::io::{self, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.as_slice() {
        [_, cmd, path] if cmd == "run"       => run_file(path),
        [_, cmd, path] if cmd == "typecheck" => typecheck_file(path),
        [_, cmd] if cmd == "console" => run_repl(),
        _ => {
            eprintln!("Usage: sapphire [run <file.spr> | typecheck <file.spr> | console]");
            std::process::exit(1);
        }
    }
}

fn run_file(path: &str) {
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error reading '{}': {}", path, e);
            std::process::exit(1);
        }
    };
    let env = interpreter::global_env();
    let tokens = lexer::Lexer::new(&source).scan_tokens();
    match parser::Parser::new(tokens).parse() {
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
        Ok(stmts) => {
            for stmt in stmts {
                if let Err(e) = interpreter::execute(stmt, env.clone()) {
                    eprintln!("{}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}

fn typecheck_file(path: &str) {
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => { eprintln!("error reading '{}': {}", path, e); std::process::exit(1); }
    };
    let tokens = lexer::Lexer::new(&source).scan_tokens();
    match parser::Parser::new(tokens).parse() {
        Err(e) => { eprintln!("{}", e); std::process::exit(1); }
        Ok(stmts) => {
            let errors = typechecker::TypeChecker::check(&stmts);
            if errors.is_empty() {
                println!("No type errors found.");
            } else {
                for e in &errors { eprintln!("{}", e); }
                std::process::exit(1);
            }
        }
    }
}

fn run_repl() {
    println!("Sapphire 0.1.0 — :q to quit");

    let env = interpreter::global_env();

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
