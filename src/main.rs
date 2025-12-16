mod aot_backend;
mod cli;
mod compiler;
mod compiler_settings;
mod diagnostics;
mod lexer;
mod parser;
mod transposer;

use crate::cli::Args;
use chumsky::{IterParser, Parser};
use clap::Parser as CliParser;
use log::debug;
use owo_colors::OwoColorize;

fn main() {
    let cli_args = Args::parse();

    if cli_args.verbose {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Debug)
            .init();
    } else {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Off)
            .init();
    }

    let file_path = cli_args
        .file
        .to_str()
        .expect("{file_path:?} is not valid path!");

    let output_path = cli_args
        .output
        .to_str()
        .expect("{file_path:?} is not valid path!");

    let code_text =
        std::fs::read_to_string(file_path).expect("Failed to read source file {file_path:?}");

    let rows: Vec<&str> = code_text.lines().collect();

    if cli_args.transpose {
        let transposed = transposer::transpose(rows, true);

        for line in transposed {
            println!("{}", line);
        }
        std::process::exit(0);
    }

    let transposed = transposer::transpose(rows, false);
    let code_text = transposed.join("\n");

    let tokens = match lexer::tokenize(&code_text) {
        Ok(tokens) => tokens,
        Err(err) => {
            diagnostics::emit_lexer_error(&err, file_path, &code_text);
            std::process::exit(1);
        }
    };

    let parser = parser::parser_stmt().repeated().collect::<Vec<_>>();
    let ast_unprocesed = parser.parse(&tokens);

    if let Some(err) = ast_unprocesed.errors().next() {
        diagnostics::emit_parser_error(err, &tokens);
        std::process::exit(1);
    }

    let ast = ast_unprocesed.unwrap();

    debug!("\n{ast:#?}");

    let settings = compiler_settings::CompilerSettings::new().unwrap();
    let mut backend = aot_backend::AOTBackend::new(&settings, output_path).unwrap();
    let mut compiler = compiler::IRCompiler::new();

    let triple = settings.target_triple();
    println!(
        "{}\n    {} ({}) - {} / {}\n",
        "Building for:".black().on_white(),
        triple.operating_system,
        triple.architecture,
        triple.environment,
        triple.binary_format
    );

    compiler
        .compile_program(backend.module_mut(), ast)
        .expect("Compilation error");

    backend.finalize().expect("Failed to finalize program");

    println!("{}", "Build successful!".green());
}
