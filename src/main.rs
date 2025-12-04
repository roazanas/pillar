use ariadne::{Color, Label, Report, ReportKind, Source};
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
    let file_path = "/home/rznz/dev_proj/rust/pillar/test.rplr";
    let test_code = std::fs::read_to_string(file_path).unwrap();
    let tokens = match lexer::tokenize(&test_code) {
        Ok(tokens) => tokens,
        Err(err) => {
            Report::build(ReportKind::Error, (file_path, err.span.clone()))
                .with_message("Lexer error")
                .with_label(
                    Label::new((file_path, err.span.clone()))
                        .with_message(format!("Unexpected character: {}", err.invalid_text))
                        .with_color(Color::Red),
                )
                .finish()
                .eprint((file_path, Source::from(&test_code)))
                .unwrap();

            std::process::exit(1);
        }
    };

    let parser = parser::parser_stmt();
    let ast_unprocesed = parser.parse(&tokens);

    if let Some(err) = ast_unprocesed.errors().next() {
        let span = err.span();
        eprintln!(
            "Syntax error:\n  found '{:?}' at {:?} ('{:?}' -> '{:?}')\n  expected: {}",
            err.found().unwrap(),
            span,
            tokens[span.start - 1],
            tokens[span.end - 1],
            err.expected()
                .map(|e| format!("{e:?}"))
                .collect::<Vec<_>>()
                .join(" or "),
        );
        std::process::exit(1);
    }

    let ast = ast_unprocesed.unwrap();

    debug!("\n{ast:#?}");

    let settings = compiler_settings::CompilerSettings::new().unwrap();
    let mut backend = aot_backend::AOTBackend::new(&settings, "output").unwrap();
    let mut compiler = compiler::IRCompiler::new();

    println!("Building for: \n{:#?}", settings.target_triple());

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

    println!("Build successful!");
}
