use logos::Logos;

#[derive(Logos, Debug, PartialEq)]
#[logos(skip r"[ \t\n\f\r]+")]
pub enum Token {
    // Ключевые слова
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

    // Литералы
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

    // Идентификаторы
    #[regex(r"[a-zA-Z_]\w*")]
    Identifier,

    // Операторы
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

    // Диапазоны
    #[token("..")]
    RangeExclusive,
    #[token("..=")]
    RangeInclusive,

    // Стрелки
    #[token("->")]
    Arrow,
    #[token("=>")]
    FatArrow,

    // Разделители
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
