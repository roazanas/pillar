use crate::{
    diagnostics::{Diagnostic, Result},
    source::{SourceFile, Span},
};
use logos::Logos;
use serde::Serialize;
#[derive(Logos, Clone, Debug, PartialEq, Serialize)]
pub enum Kind {
    #[regex(r"[ \n\r\f]+")]
    Space,
    #[regex(r"//[^\n]*", allow_greedy = true)]
    LineComment,
    #[regex(r"/\*([^*]|\*+[^*/])*\*+/")]
    BlockComment,
    #[token("FN")]
    Fn,
    #[token("LT")]
    Let,
    #[token("RET")]
    Ret,
    #[token("IF")]
    If,
    #[token("EL")]
    Else,
    #[token("WH")]
    While,
    #[token("break")]
    Break,
    #[token("continue")]
    Continue,
    #[token("BRK")]
    Brk,
    #[token("CONT")]
    Cont,
    #[token("IM")]
    Import,
    #[token("AS")]
    As,
    #[token("PUB")]
    Pub,
    #[token("ST")]
    Struct,
    #[token("INPI")]
    InputInt,
    #[token("INPF")]
    InputFloat,
    #[token("INPS")]
    InputString,
    #[token("OUTI")]
    OutputInt,
    #[token("OUTF")]
    OutputFloat,
    #[token("OUTS")]
    OutputString,
    #[token("true")]
    True,
    #[token("false")]
    False,
    #[token("int")]
    IntType,
    #[token("float")]
    FloatType,
    #[token("bool")]
    BoolType,
    #[token("str")]
    StrType,
    #[token("unit")]
    UnitType,
    #[regex(r"[0-9]+", callback = token_text)]
    Int(String),
    #[regex(r"[0-9]+(\.[0-9]+([eE][+-]?[0-9]+)?|[eE][+-]?[0-9]+)", callback = token_text)]
    Float(String),
    #[regex(r#""([^"\\\n\r]|\\[^\n\r])*""#, callback = token_text)]
    Str(String),
    #[regex(r"[a-zA-Z_][a-zA-Z_0-9]*", callback = token_text)]
    Ident(String),
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
    Eq,
    #[token("!=")]
    Ne,
    #[token("<")]
    Lt,
    #[token("<=")]
    Le,
    #[token(">")]
    Gt,
    #[token(">=")]
    Ge,
    #[token("!")]
    Not,
    #[token("&&")]
    And,
    #[token("||")]
    Or,
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token(",")]
    Comma,
    #[token(":")]
    Colon,
    #[token("::")]
    Scope,
    #[token(".")]
    Dot,
    #[token("~")]
    End,
}
impl Kind {
    pub fn trivia(&self) -> bool {
        matches!(self, Self::Space | Self::LineComment | Self::BlockComment)
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct Token {
    pub kind: Kind,
    pub span: Span,
    pub text: String,
}
pub fn tokenize(source: &SourceFile) -> Result<Vec<Token>> {
    if let Some(message) = &source.layout_error {
        return Err(Diagnostic::error("E0003", message, source.span(0..0)));
    }
    if let Some(span) = source.tab_span() {
        return Err(Diagnostic::error(
            "E0001",
            "tabs are not allowed; use spaces or the \\t escape",
            span,
        ));
    }
    let mut tokens = Vec::new();
    for (kind, range) in Kind::lexer(&source.logical).spanned() {
        let span = source.span(range.clone());
        let kind = kind.map_err(|_| {
            Diagnostic::error(
                "E0002",
                format!(
                    "unexpected character or unterminated literal: {:?}",
                    &source.logical[range.clone()]
                ),
                span,
            )
        })?;
        tokens.push(Token {
            kind,
            span,
            text: source.logical[range].into(),
        });
    }
    Ok(tokens)
}

fn token_text(lex: &mut logos::Lexer<'_, Kind>) -> String {
    lex.slice().to_owned()
}
