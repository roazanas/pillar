use log::debug;
use logos::Logos;

#[derive(Logos, Debug, PartialEq)]
#[logos(skip r"[ \t\n\f\r]+")]
pub enum Token {
    #[token("LT")]
    KeywordLet,
    #[token("FN")]
    KeywordFn,
    #[token("RET")]
    KeywordReturn,
    #[token("IF")]
    KeywordIf,
    #[token("EI")]
    KeywordElseIf,
    #[token("EL")]
    KeywordElse,
    #[token("FR")]
    KeywordFor,
    #[token("BY")]
    KeywordBy,
    #[token("IN")]
    KeywordIn,
    #[token("WH")]
    KeywordWhile,
    #[token("BR")]
    KeywordBreak,
    #[token("CN")]
    KeywordContinue,
    #[token("LP")]
    KeywordLoop,

    #[token("true")]
    BooleanTrue,
    #[token("false")]
    BooleanFalse,
    #[regex(r"\d+")]
    IntLiteral,
    #[regex(r"\d+\.\d+")]
    FloatLiteral,
    #[regex(r#""[^"]*""#)]
    StringLiteral,

    #[regex(r"[a-zA-Z_]\w*")]
    Identifier,

    #[token("=")]
    Assign,
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("%")]
    Percent,
    #[token("==")]
    Equal,
    #[token("!=")]
    NotEqual,
    #[token("<")]
    Less,
    #[token("<=")]
    LessEqual,
    #[token(">")]
    Greater,
    #[token(">=")]
    GreaterEqual,
    #[token("&&")]
    LogicalAnd,
    #[token("||")]
    LogicalOr,
    #[token("!")]
    LogicalNot,

    #[token("..")]
    RangeExclusive,
    #[token("..=")]
    RangeInclusive,

    #[token("->")]
    Arrow,
    #[token("=>")]
    FatArrow,

    #[token("(")]
    LeftParen,
    #[token(")")]
    RightParen,
    #[token("{")]
    LeftBrace,
    #[token("}")]
    RightBrace,
    #[token("[")]
    LeftBracket,
    #[token("]")]
    RightBracket,
    #[token(",")]
    Comma,
    #[token(".")]
    Dot,
    #[token(":")]
    Colon,
}

pub fn tokenize(input: &str) -> Vec<Token> {
    let mut lex = Token::lexer(input);
    let mut tokens: Vec<Token> = Vec::new();

    while let Some(result) = lex.next() {
        match result {
            Ok(tok) => {
                debug!("{:<15} => {:>15?}", lex.slice(), tok);
                tokens.push(tok)
            }
            Err(e) => panic!("Lexer error occured: {:?}", e),
        }
    }
    tokens
}
