use chumsky::Parser;

mod lexer;
mod parser;

fn main() {
    env_logger::init();
    // let test_code =
    //     std::fs::read_to_string("/home/rznz/dev_proj/rust/pillar/example.rplr").unwrap();
    let tokens = lexer::tokenize("1000-10 * 2 <= 5 + 2*3");
    let parser = parser::parser();
    println!("{:#?}", parser.parse(&tokens[..]));
    // let tokens = lexer::tokenize(&test_code);
}
