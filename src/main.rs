mod aot_backend;
mod compiler;
mod compiler_settings;
mod diagnostics;
mod lexer;
mod parser;

use chumsky::Parser;
use log::debug;

fn main() {
    env_logger::init();
    // let test_code =
    //     std::fs::read_to_string("/home/rznz/dev_proj/rust/pillar/example.rplr").unwrap();
    let file_path = "/home/rznz/dev_proj/rust/pillar/test.rplr";
    let test_code = std::fs::read_to_string(file_path).unwrap();
    let tokens = match lexer::tokenize(&test_code) {
        Ok(tokens) => tokens,
        Err(err) => {
            diagnostics::emit_lexer_error(&err, file_path, &test_code);
            std::process::exit(1);
        }
    };

    let parser = parser::parser_stmt();
    let ast_unprocesed = parser.parse(&tokens);

    if let Some(err) = ast_unprocesed.errors().next() {
        diagnostics::emit_parser_error(err, &tokens);
        std::process::exit(1);
    }

    let ast = ast_unprocesed.unwrap();

    debug!("\n{ast:#?}");

    let settings = compiler_settings::CompilerSettings::new().unwrap();
    let mut backend = aot_backend::AOTBackend::new(&settings, "output").unwrap();
    let mut compiler = compiler::IRCompiler::new();

    println!("Building for: \n{:#?}", settings.target_triple());

    compiler
        .compile_program(backend.module_mut(), ast)
        .expect("Compilation error");

    backend.finalize().unwrap();

    println!("Build successful!");
}
