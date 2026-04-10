use sapphire::{compiler, interpreter, lexer, parser, typechecker, value, vm};
use std::io::{self, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.as_slice() {
        [_, cmd, path] if cmd == "run"       => run_file(path),
        [_, cmd, path] if cmd == "vm"        => run_file_vm(path),
        [_, cmd, path] if cmd == "typecheck" => typecheck_file(path),
        [_, cmd] if cmd == "console" => run_repl(),
        [_, cmd] if cmd == "version" => println!("sapphire {}", env!("CARGO_PKG_VERSION")),
        _ => {
            eprintln!("Usage: sapphire <command>\n");
            eprintln!("Commands:");
            eprintln!("  run <file.spr>       Run a file using the tree-walk interpreter");
            eprintln!("  vm <file.spr>        Run a file using the experimental bytecode VM");
            eprintln!("  typecheck <file.spr> Type-check a file");
            eprintln!("  console              Start the REPL");
            eprintln!("  version              Print the version");
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
        Ok(exprs) => {
            for expr in exprs {
                if let Err(e) = interpreter::execute(expr, env.clone()) {
                    eprintln!("{}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}

fn run_file_vm(path: &str) {
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error reading '{}': {}", path, e);
            std::process::exit(1);
        }
    };
    let tokens = lexer::Lexer::new(&source).scan_tokens();
    let exprs = match parser::Parser::new(tokens).parse() {
        Ok(s) => s,
        Err(e) => { eprintln!("{}", e); std::process::exit(1); }
    };
    let func = match compiler::compile(&exprs) {
        Ok(f) => f,
        Err(e) => { eprintln!("{}", e); std::process::exit(1); }
    };
    let mut vm = vm::Vm::new(func);
    if let Err(e) = vm.load_stdlib() {
        eprintln!("stdlib error: {}", e);
        std::process::exit(1);
    }
    if let Err(e) = vm.run() {
        eprintln!("{}", e);
        std::process::exit(1);
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
        Ok(exprs) => {
            let errors = typechecker::TypeChecker::check(&exprs);
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
    println!("Sapphire 0.1.0 — Ctrl+D to quit");

    let env = interpreter::global_env();

    loop {
        print!("> ");
        io::stdout().flush().unwrap();

        let mut line = String::new();
        match io::stdin().read_line(&mut line) {
            Ok(0) | Err(_) => { println!(); break; }
            _ => {}
        }

        let source = line.trim();
        if source.is_empty() {
            continue;
        }

        let tokens = lexer::Lexer::new(source).scan_tokens();
        match parser::Parser::new(tokens).parse() {
            Err(e) => eprintln!("{}", e),
            Ok(exprs) => {
                for expr in exprs {
                    match interpreter::execute(expr, env.clone()) {
                        Ok(Some(result)) if result != value::Value::Nil => println!("{}", result),
                        Ok(_) => {}
                        Err(e) => { eprintln!("{}", e); break; }
                    }
                }
            }
        }
    }
}
