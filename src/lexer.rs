use log::debug;
use logos::Logos;

#[derive(Logos, Debug, PartialEq, Clone)]
#[logos(skip r"[ \t\n\f\r]+")]
pub enum Token<'input> {
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
    #[token("INPI")]
    KeywordInputInt,
    #[token("INPF")]
    KeywordInputFloat,
    #[token("OUTI")]
    KeywordOutputInt,
    #[token("OUTF")]
    KeywordOutputFloat,

    #[token("true")]
    BooleanTrue,
    #[token("false")]
    BooleanFalse,
    #[regex(r"\d+", |lex| lex.slice().parse().ok())]
    IntLiteral(i64),
    #[regex(r"\d+\.\d+", |lex| lex.slice().parse().ok())]
    FloatLiteral(f64),
    #[regex(r#""[^"]*""#, |lex| lex.slice())]
    StringLiteral(&'input str),

    #[token("int")]
    TypeInt,
    #[token("float")]
    TypeFloat,
    #[token("bool")]
    TypeBool,
    #[token("str")]
    TypeString,

    #[regex(r"[a-zA-Z_][a-zA-Z_\d]*", |lex| lex.slice())]
    Identifier(&'input str),

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
    #[token("~")]
    Tilda,
}

pub fn tokenize(input: &str) -> Result<Vec<Token<'_>>, LexError> {
    let mut lex = Token::lexer(input);
    let mut tokens: Vec<Token> = Vec::new();

    while let Some(result) = lex.next() {
        match result {
            Ok(tok) => {
                debug!("{:<15} => {:?}", lex.slice(), tok);
                tokens.push(tok)
            }
            Err(_) => {
                return Err(LexError {
                    span: lex.span(),
                    invalid_text: lex.slice().to_string(),
                });
            }
        }
    }

    Ok(tokens)
}

#[derive(Debug)]
pub struct LexError {
    pub span: std::ops::Range<usize>,
    pub invalid_text: String,
}
