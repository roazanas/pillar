use chumsky::Parser;

mod lexer;
mod parser;

fn main() {
    env_logger::init();
    // let test_code =
    //     std::fs::read_to_string("/home/rznz/dev_proj/rust/pillar/example.rplr").unwrap();
    let tokens = lexer::tokenize(
        "FN main() {
LT x: int = 2*3;
LT y: int = 9+2;
LT res: int = x+y;
}",
    );
    let parser = parser::parser_stmt();
    println!("{:#?}", parser.parse(&tokens[..]));
    // let tokens = lexer::tokenize(&test_code);
}
