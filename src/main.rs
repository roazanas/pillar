mod lexer;
use crate::lexer::Token;
use logos::Logos;

fn main() {
    let mut lex = Token::lexer("FR i IN 1..=2: abc = i");

    while let Some(result) = lex.next() {
        match result {
            Ok(tok) => println!("{:?}: {}", tok, lex.slice()),
            Err(e) => panic!("Lexer error occured: {:?}", e),
        }
    }
}
