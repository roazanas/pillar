use crate::lexer::Token;
use chumsky::pratt::*;
use chumsky::prelude::*;

#[derive(Debug, Clone)]
pub enum Type {
    Int,
    Float,
    Boolean,
    String,
}

#[derive(Debug, Clone)]
pub enum Expression<'src> {
    Int(i64),
    Float(f64),
    Boolean(bool),
    String(&'src str),
    Identifier(&'src str),

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
        value: Expression<'src>,
    },
    Fn {
        name: &'src str,
        arguments: Vec<TypedVar<'src>>,
        code: Block<'src>,
    },
    Ret {
        value: Expression<'src>,
    },
    If {
        condition: Expression<'src>,
        then_branch: Block<'src>,
        else_branch: Option<Block<'src>>,
    },
}

#[derive(Debug, Clone)]
pub struct Block<'src> {
    pub statements: Vec<Statement<'src>>,
}

#[derive(Debug, Clone)]
pub struct TypedVar<'src> {
    pub variables: (Type, &'src str),
}

pub fn parser_expr<'src>()
-> impl Parser<'src, &'src [Token<'src>], Expression<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    let literal = select! {
        Token::IntLiteral(n) => Expression::Int(n),
        Token::FloatLiteral(n) => Expression::Float(n),
        Token::BooleanTrue => Expression::Boolean(true),
        Token::BooleanFalse => Expression::Boolean(false),
        Token::StringLiteral(s) => Expression::String(s),
        Token::Identifier(s) => Expression::Identifier(s),
    };

    let op_add = just(Token::Plus);
    let op_sub = just(Token::Minus);
    let op_mul = just(Token::Star);
    let op_div = just(Token::Slash);
    let op_mod = just(Token::Percent);
    let op_eq = just(Token::Equal);
    let op_nq = just(Token::NotEqual);
    let op_ls = just(Token::Less);
    let op_le = just(Token::LessEqual);
    let op_gr = just(Token::Greater);
    let op_ge = just(Token::GreaterEqual);

    literal.pratt((
        infix(left(1), op_eq, |l, _, r, _| Expression::Equal {
            lho: Box::new(l),
            rho: Box::new(r),
        }),
        infix(left(1), op_nq, |l, _, r, _| Expression::NotEqual {
            lho: Box::new(l),
            rho: Box::new(r),
        }),
        infix(left(1), op_ls, |l, _, r, _| Expression::Less {
            lho: Box::new(l),
            rho: Box::new(r),
        }),
        infix(left(1), op_le, |l, _, r, _| Expression::LessEqual {
            lho: Box::new(l),
            rho: Box::new(r),
        }),
        infix(left(1), op_gr, |l, _, r, _| Expression::Greater {
            lho: Box::new(l),
            rho: Box::new(r),
        }),
        infix(left(1), op_ge, |l, _, r, _| Expression::GreaterEqual {
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

pub fn parser_stmt<'src>()
-> impl Parser<'src, &'src [Token<'src>], Statement<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    recursive(|stmt_parser| {
        let type_parser = select! {
            Token::TypeInt => Type::Int,
            Token::TypeFloat => Type::Float,
            Token::TypeBool => Type::Boolean,
            Token::TypeString => Type::String,
        };

        let ident_parser = select! {
            Token::Identifier(s) => s,
        };

        let typed_var = ident_parser
            .then_ignore(just(Token::Colon))
            .then(type_parser)
            .map(|(name, typ)| TypedVar {
                variables: (typ, name),
            });

        let block = just(Token::LeftBrace)
            .ignore_then(stmt_parser.clone().repeated().collect::<Vec<_>>())
            .then_ignore(just(Token::RightBrace))
            .map(|statements| Block { statements });

        let stmt_let = just(Token::KeywordLet)
            .ignore_then(ident_parser)
            .then_ignore(just(Token::Colon))
            .then(type_parser)
            .then_ignore(just(Token::Assign))
            .then(parser_expr().boxed())
            .then_ignore(just(Token::Semicolon))
            .map(|((name, _typ), expr)| Statement::Let { name, value: expr });

        let stmt_fn = just(Token::KeywordFn)
            .ignore_then(ident_parser)
            .then_ignore(just(Token::LeftParen))
            .then(
                typed_var
                    .separated_by(just(Token::Comma))
                    .collect::<Vec<_>>(),
            )
            .then_ignore(just(Token::RightParen))
            .then(block)
            .map(|((name, arguments), code)| Statement::Fn {
                name,
                arguments,
                code,
            });

        let stmt_ret = just(Token::KeywordReturn)
            .ignore_then(parser_expr().boxed())
            .then_ignore(just(Token::Semicolon))
            .map(|value| Statement::Ret { value });

        choice((stmt_let, stmt_fn, stmt_ret))
    })
}
