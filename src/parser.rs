use crate::lexer::Token;
use chumsky::pratt::*;
use chumsky::prelude::*;

#[derive(Debug, Clone)]
pub enum Expression<'src> {
    Int(i64),
    Float(f64),
    Boolean(bool),
    String(&'src str),

    Add { lho: Box<Self>, rho: Box<Self> },
    Sub { lho: Box<Self>, rho: Box<Self> },
    Mul { lho: Box<Self>, rho: Box<Self> },
    Div { lho: Box<Self>, rho: Box<Self> },
    Mod { lho: Box<Self>, rho: Box<Self> },

    Equal { lho: Box<Self>, rho: Box<Self> },
    NotEqual { lho: Box<Self>, rho: Box<Self> },
    Less { lho: Box<Self>, rho: Box<Self> },
    LessEqual { lho: Box<Self>, rho: Box<Self> },
    Greater { lho: Box<Self>, rho: Box<Self> },
    GreaterEqual { lho: Box<Self>, rho: Box<Self> },
}

#[derive(Debug, Clone)]
pub enum Statement<'src> {
    Let {
        name: &'src str,
        value: Box<Self>,
    },
    If {
        condition: Box<Self>,
        then_branch: Box<Self>,
        else_branch: Option<Box<Self>>,
    },
}

pub fn parser<'src>() -> impl Parser<'src, &'src [Token<'src>], Expression<'src>> {
    let literal = select! {
        Token::IntLiteral(n) => Expression::Int(n),
        Token::FloatLiteral(n) => Expression::Float(n),
        Token::BooleanTrue => Expression::Boolean(true),
        Token::BooleanFalse => Expression::Boolean(false),
        Token::StringLiteral(s) => Expression::String(s),
    };

    let op_add = select! { Token::Plus => () };
    let op_sub = select! { Token::Minus => () };
    let op_mul = select! { Token::Star => () };
    let op_div = select! { Token::Slash => () };
    let op_mod = select! { Token::Percent => () };
    let op_eq = select! { Token::Equal => () };
    let op_neq = select! { Token::NotEqual => () };
    let op_lt = select! { Token::Less => () };
    let op_lte = select! { Token::LessEqual => () };
    let op_gt = select! { Token::Greater => () };
    let op_gte = select! { Token::GreaterEqual => () };

    literal.pratt((
        infix(left(1), op_eq, |l, _, r, _| Expression::Equal {
            lho: Box::new(l),
            rho: Box::new(r),
        }),
        infix(left(1), op_neq, |l, _, r, _| Expression::NotEqual {
            lho: Box::new(l),
            rho: Box::new(r),
        }),
        infix(left(1), op_lt, |l, _, r, _| Expression::Less {
            lho: Box::new(l),
            rho: Box::new(r),
        }),
        infix(left(1), op_lte, |l, _, r, _| Expression::LessEqual {
            lho: Box::new(l),
            rho: Box::new(r),
        }),
        infix(left(1), op_gt, |l, _, r, _| Expression::Greater {
            lho: Box::new(l),
            rho: Box::new(r),
        }),
        infix(left(1), op_gte, |l, _, r, _| Expression::GreaterEqual {
            lho: Box::new(l),
            rho: Box::new(r),
        }),
        infix(left(2), op_add, |l, _, r, _| Expression::Add {
            lho: Box::new(l),
            rho: Box::new(r),
        }),
        infix(left(2), op_sub, |l, _, r, _| Expression::Sub {
            lho: Box::new(l),
            rho: Box::new(r),
        }),
        infix(left(3), op_mul, |l, _, r, _| Expression::Mul {
            lho: Box::new(l),
            rho: Box::new(r),
        }),
        infix(left(3), op_div, |l, _, r, _| Expression::Div {
            lho: Box::new(l),
            rho: Box::new(r),
        }),
        infix(left(3), op_mod, |l, _, r, _| Expression::Mod {
            lho: Box::new(l),
            rho: Box::new(r),
        }),
    ))
}
