use std::path::PathBuf;

use wasm_bindgen::prelude::*;

use crate::{compiler, lexer, parser, vm};

#[wasm_bindgen]
pub struct RunResult {
    output: String,
    error: Option<String>,
}

#[wasm_bindgen]
impl RunResult {
    pub fn output(&self) -> String {
        self.output.clone()
    }

    pub fn error(&self) -> Option<String> {
        self.error.clone()
    }

    pub fn is_ok(&self) -> bool {
        self.error.is_none()
    }
}

#[wasm_bindgen]
pub fn run_sapphire(source: &str) -> RunResult {
    console_error_panic_hook::set_once();

    let tokens = lexer::Lexer::new(source).scan_tokens();
    let stmts = match parser::Parser::new(tokens).parse() {
        Ok(s) => s,
        Err(e) => {
            return RunResult {
                output: String::new(),
                error: Some(e.to_string()),
            }
        }
    };
    let func = match compiler::compile(&stmts) {
        Ok(f) => f,
        Err(e) => {
            return RunResult {
                output: String::new(),
                error: Some(e.to_string()),
            }
        }
    };

    let mut machine = vm::Vm::new(func, PathBuf::new());
    machine.output = Some(Vec::new());

    if let Err(e) = machine.load_stdlib() {
        return RunResult {
            output: String::new(),
            error: Some(e.to_string()),
        };
    }

    match machine.run() {
        Ok(_) => RunResult {
            output: machine.output.unwrap_or_default().join("\n"),
            error: None,
        },
        Err(e) => RunResult {
            output: machine.output.unwrap_or_default().join("\n"),
            error: Some(e.to_string()),
        },
    }
}
