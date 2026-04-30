#[cfg(feature = "cli")]
use rustyline::{DefaultEditor, error::ReadlineError};
use sapphire::{compiler, lexer, parser, token::TokenKind, typechecker, vm};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.as_slice() {
        [_, cmd, path, ..] if cmd == "run" => run_file(path),
        [_, cmd, path] if cmd == "typecheck" => typecheck_file(path),
        [_, cmd, path] if cmd == "test" => run_tests(path),
        [_, cmd] if cmd == "test" => run_tests("."),
        #[cfg(feature = "cli")]
        [_, cmd] if cmd == "console" => run_repl(),
        [_, cmd] if cmd == "version" => println!("sapphire {}", env!("CARGO_PKG_VERSION")),
        _ => {
            eprintln!("Usage: sapphire <command>\n");
            eprintln!("Commands:");
            eprintln!("  run <file.spr>       Run a Sapphire file");
            eprintln!("  typecheck <file.spr> Type-check a file");
            eprintln!("  test [path]          Run tests (file or directory)");
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
    let tokens = lexer::Lexer::new(&source).scan_tokens();
    let exprs = match parser::Parser::new(tokens).parse() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };
    if !report_type_errors(&exprs) {
        std::process::exit(1);
    }
    let func = match compiler::compile(&exprs) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };
    let current_dir = std::path::Path::new(path)
        .canonicalize()
        .unwrap_or_else(|_| std::path::PathBuf::from(path))
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let mut vm = vm::Vm::new(func, current_dir);
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
        Err(e) => {
            eprintln!("error reading '{}': {}", path, e);
            std::process::exit(1);
        }
    };
    let tokens = lexer::Lexer::new(&source).scan_tokens();
    match parser::Parser::new(tokens).parse() {
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
        Ok(exprs) => {
            if report_type_errors(&exprs) {
                println!("No type errors found.");
            } else {
                std::process::exit(1);
            }
        }
    }
}

fn report_type_errors(exprs: &[sapphire::ast::Expr]) -> bool {
    let errors = typechecker::TypeChecker::check(exprs);
    if errors.is_empty() {
        true
    } else {
        for e in &errors {
            eprintln!("{}", e);
        }
        false
    }
}

fn collect_test_files(path: &str) -> Vec<std::path::PathBuf> {
    let p = std::path::Path::new(path);
    if p.is_file() {
        return vec![p.to_path_buf()];
    }
    let mut files = Vec::new();
    collect_test_files_recursive(p, &mut files);
    files.sort();
    files
}

fn collect_test_files_recursive(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_test_files_recursive(&path, out);
        } else if path.extension().is_some_and(|e| e == "spr")
            && path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with("_test.spr"))
        {
            out.push(path);
        }
    }
}

fn run_tests(path: &str) {
    let test_files = collect_test_files(path);
    if test_files.is_empty() {
        eprintln!("No test files found in '{}'", path);
        std::process::exit(1);
    }

    let start_time = std::time::Instant::now();
    let mut total = 0usize;
    let mut failures: Vec<String> = Vec::new();
    let mut dots = String::new();

    for file_path in &test_files {
        let source = match std::fs::read_to_string(file_path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error reading '{}': {}", file_path.display(), e);
                std::process::exit(1);
            }
        };
        let tokens = lexer::Lexer::new(&source).scan_tokens();
        let exprs = match parser::Parser::new(tokens).parse() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        };
        if !report_type_errors(&exprs) {
            std::process::exit(1);
        }
        let func = match compiler::compile(&exprs) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        };
        let current_dir = file_path
            .canonicalize()
            .unwrap_or_else(|_| file_path.to_path_buf())
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::path::PathBuf::from("."));

        let mut machine = vm::Vm::new(func, current_dir);
        if let Err(e) = machine.load_stdlib() {
            eprintln!("stdlib error: {}", e);
            std::process::exit(1);
        }
        if let Err(e) = machine.run() {
            eprintln!("{}", e);
            std::process::exit(1);
        }

        let test_classes = machine.collect_test_classes();
        for (class_name, tests) in test_classes {
            for (label, method) in &tests {
                total += 1;
                match machine.run_single_test(&class_name, method) {
                    Ok(()) => dots.push('.'),
                    Err(msg) => {
                        dots.push('F');
                        failures.push(format!("  {}#{} — {}", class_name, label, msg));
                    }
                }
            }
        }
    }

    println!("{}\n", dots);
    if !failures.is_empty() {
        println!("Failures:");
        for f in &failures {
            println!("{}", f);
        }
        println!();
    }
    let elapsed = start_time.elapsed();
    let elapsed_secs = elapsed.as_secs_f64();
    let tests_per_sec = if elapsed_secs > 0.0 {
        total as f64 / elapsed_secs
    } else {
        0.0
    };
    println!(
        "{} {}, {} {} ({:.2}s, {:.0} tests/sec)",
        total,
        if total == 1 { "test" } else { "tests" },
        failures.len(),
        if failures.len() == 1 {
            "failure"
        } else {
            "failures"
        },
        elapsed_secs,
        tests_per_sec
    );
    if !failures.is_empty() {
        std::process::exit(1);
    }
}

#[cfg(feature = "cli")]
fn is_input_complete(source: &str) -> bool {
    let tokens = lexer::Lexer::new(source).scan_tokens();
    let mut depth: i32 = 0;
    let mut begin_depth: i32 = 0;
    for token in &tokens {
        match &token.kind {
            TokenKind::LeftBrace | TokenKind::LeftParen | TokenKind::LeftBracket => depth += 1,
            TokenKind::RightBrace | TokenKind::RightParen | TokenKind::RightBracket => depth -= 1,
            TokenKind::Begin => begin_depth += 1,
            TokenKind::End => begin_depth -= 1,
            _ => {}
        }
    }
    depth <= 0 && begin_depth <= 0
}

#[cfg(feature = "cli")]
fn run_repl() {
    println!(
        "Sapphire {} — type quit, or press Ctrl+D to quit",
        env!("CARGO_PKG_VERSION")
    );

    let mut vm = vm::Vm::new_repl();
    if let Err(e) = vm.load_stdlib() {
        eprintln!("stdlib error: {}", e);
        std::process::exit(1);
    }

    let mut rl = DefaultEditor::new().expect("failed to create editor");

    loop {
        let first_line = match rl.readline("> ") {
            Ok(line) => line,
            Err(ReadlineError::Interrupted) => continue,
            Err(ReadlineError::Eof) => {
                println!();
                break;
            }
            Err(e) => {
                eprintln!("error: {}", e);
                break;
            }
        };

        if first_line.trim().is_empty() {
            continue;
        }

        let mut source = first_line;

        while !is_input_complete(&source) {
            match rl.readline(".. ") {
                Ok(line) => {
                    source.push('\n');
                    source.push_str(&line);
                }
                Err(_) => break,
            }
        }

        rl.add_history_entry(&source).ok();

        let trimmed = source.trim();
        if trimmed == "quit" {
            break;
        }
        let tokens = lexer::Lexer::new(trimmed).scan_tokens();
        let exprs = match parser::Parser::new(tokens).parse() {
            Err(e) => {
                eprintln!("{}", e);
                continue;
            }
            Ok(e) => e,
        };
        if !report_type_errors(&exprs) {
            continue;
        }
        let func = match compiler::compile_repl(&exprs) {
            Err(e) => {
                eprintln!("{}", e);
                continue;
            }
            Ok(f) => f,
        };
        match vm.eval(func) {
            Ok(Some(result)) if result.to_string() != "nil" => println!("{}", result),
            Ok(_) => {}
            Err(e) => eprintln!("{}", e),
        }
    }
}
