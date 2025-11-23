use crate::lexer::Token;
use chumsky::prelude::*;

pub fn parser<'src>() -> impl Parser<'src, &'src [Token<'src>], ()> {
    end()
}
