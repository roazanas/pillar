use std::process::Command;

use chumsky::Parser;
use cranelift::prelude::types;
use log::debug;

use crate::parser::Statement;

mod aot_backend;
mod compiler;
mod compiler_settings;
mod lexer;
mod parser;

fn main() {
    env_logger::init();
    // let test_code =
    //     std::fs::read_to_string("/home/rznz/dev_proj/rust/pillar/example.rplr").unwrap();
    let tokens = lexer::tokenize("FN main() {RET 1+2 + 4 +2;}");
    let parser = parser::parser_stmt();
    let ast = parser.parse(&tokens).unwrap();
    debug!("{ast:#?}");

    let settings = compiler_settings::CompilerSettings::new().unwrap();
    let mut backend = aot_backend::AOTBackend::new(&settings, "out.o").unwrap();
    let mut compiler = compiler::IRCompiler::new();

    match ast {
        Statement::Fn {
            name,
            arguments,
            code,
        } => {
            let entry_params: Vec<cranelift::prelude::Type> = arguments
                .iter()
                .map(|arg| compiler::translate(&arg.variables.0))
                .collect();

            compiler
                .compile_function(
                    backend.module_mut(),
                    name,
                    &entry_params,
                    Some(types::I64),
                    code,
                )
                .expect("Compilation error")
        }
        _ => panic!("Unable to find the main function"),
    };

    backend.finalize().unwrap();

    Command::new("cc")
        .arg("out.o")
        .arg("-o")
        .arg("output")
        .status()
        .expect("Failed to link");

    println!("Build successful!");

    Command::new("rm").arg("out.o");
}
