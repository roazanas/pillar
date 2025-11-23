mod lexer;

fn main() {
    env_logger::init();
    let test_code =
        std::fs::read_to_string("/home/rznz/dev_proj/rust/pillar/example.rplr").unwrap();
    let res_tokens = lexer::tokenize(&test_code);
    println!("{res_tokens:?}");
}
