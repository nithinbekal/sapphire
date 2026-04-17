// Stdlib tests organized by class
// This mirrors the stdlib/ directory structure (int.spr, float.spr, string.spr, etc.)

use sapphire::compiler::compile;
use sapphire::lexer::Lexer;
use sapphire::parser::Parser;
use sapphire::vm::{Vm, VmValue};

fn eval(src: &str) -> VmValue {
    let tokens = Lexer::new(src).scan_tokens();
    let stmts = Parser::new(tokens).parse().expect("parse error");
    let func = compile(&stmts).expect("compile error");
    let mut vm = Vm::new(func, std::path::PathBuf::new());
    vm.load_stdlib().expect("stdlib");
    vm.run().expect("vm error").expect("empty stack")
}

#[path = "stdlib/int.rs"]
mod int;

#[path = "stdlib/float.rs"]
mod float;

#[path = "stdlib/string.rs"]
mod string;

#[path = "stdlib/list.rs"]
mod list;

#[path = "stdlib/map.rs"]
mod map;

#[path = "stdlib/range.rs"]
mod range;

#[path = "stdlib/num.rs"]
mod num;

#[path = "stdlib/object.rs"]
mod object;

#[path = "stdlib/math.rs"]
mod math;
