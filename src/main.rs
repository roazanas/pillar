use std::io::{self, BufRead};

mod lexer;

fn main() {
    env_logger::init();
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        match line {
            Ok(input) => {
                if input.is_empty() {
                    break;
                }
                lexer::tokenize(&input);
            }
            Err(_) => break,
        }
    }
}
