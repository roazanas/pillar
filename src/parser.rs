use crate::{
    diagnostics::{Diagnostic, Result},
    lexer::{Kind, Token},
    source::Span,
};
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum Type {
    Int,
    Float,
    Bool,
    Str,
    Unit,
    Array(Box<Type>),
    Named(Vec<String>),
    Struct(usize),
    Unknown,
}
impl Type {
    pub fn reference(&self) -> bool {
        matches!(self, Self::Str | Self::Array(_) | Self::Struct(_))
    }
    pub fn tag(&self) -> i64 {
        match self {
            Self::Int => 0,
            Self::Float => 1,
            Self::Bool => 2,
            Self::Str => 3,
            Self::Array(_) => 4,
            Self::Struct(_) => 5,
            _ => 6,
        }
    }
}
impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Int => write!(f, "int"),
            Self::Float => write!(f, "float"),
            Self::Bool => write!(f, "bool"),
            Self::Str => write!(f, "str"),
            Self::Unit => write!(f, "unit"),
            Self::Array(t) => write!(f, "[{t}]"),
            Self::Named(p) => write!(f, "{}", p.join("::")),
            Self::Struct(id) => write!(f, "struct#{id}"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Neg,
    Not,
}
impl Op {
    pub fn comparison(self) -> bool {
        matches!(
            self,
            Self::Eq | Self::Ne | Self::Lt | Self::Le | Self::Gt | Self::Ge
        )
    }
}
#[derive(Clone, Debug, Serialize)]
pub enum Target {
    Function(usize),
    Builtin(String),
}
#[derive(Clone, Debug, Serialize)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
    pub ty: Type,
    pub height: usize,
}
#[derive(Clone, Debug, Serialize)]
pub enum ExprKind {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    Var {
        name: String,
        id: usize,
    },
    Unary(Op, Box<Expr>),
    Binary(Op, Box<Expr>, Box<Expr>),
    Group(Box<Expr>),
    Call {
        name: Vec<String>,
        args: Vec<Expr>,
        target: Option<Target>,
    },
    Array(Vec<Expr>),
    Index(Box<Expr>, Box<Expr>),
    Field {
        base: Box<Expr>,
        name: String,
        index: usize,
    },
    Struct {
        name: Vec<String>,
        fields: Vec<(String, Expr)>,
        id: usize,
    },
}
impl Expr {
    fn new(kind: ExprKind, span: Span) -> Self {
        {
            let height = match &kind {
                ExprKind::Unary(_, e) | ExprKind::Group(e) => e.height + 1,
                ExprKind::Binary(_, a, b) | ExprKind::Index(a, b) => a.height.max(b.height) + 1,
                ExprKind::Field { base, .. } => base.height + 1,
                ExprKind::Call { args, .. } | ExprKind::Array(args) => {
                    args.iter().map(|e| e.height).max().unwrap_or(0) + 1
                }
                ExprKind::Struct { fields, .. } => {
                    fields.iter().map(|(_, e)| e.height).max().unwrap_or(0) + 1
                }
                _ => 1,
            };
            Self {
                kind,
                span,
                ty: Type::Unknown,
                height,
            }
        }
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
    pub terminates: bool,
}
#[derive(Clone, Debug, Serialize)]
pub enum StmtKind {
    Let {
        name: String,
        annotation: Option<Type>,
        value: Expr,
        id: usize,
        name_span: Span,
    },
    Assign {
        place: Expr,
        value: Expr,
    },
    Ret(Option<Expr>),
    If {
        condition: Expr,
        yes: Vec<Stmt>,
        no: Option<Vec<Stmt>>,
    },
    While {
        condition: Expr,
        body: Vec<Stmt>,
    },
    Break,
    Continue,
    Expr(Expr),
}
#[derive(Clone, Debug, Serialize)]
pub struct Param {
    pub name: String,
    pub ty: Type,
    pub span: Span,
    pub id: usize,
}
#[derive(Clone, Debug, Serialize)]
pub struct Function {
    pub name: String,
    pub name_span: Span,
    pub public: bool,
    pub params: Vec<Param>,
    pub result: Type,
    pub body: Vec<Stmt>,
    pub span: Span,
    pub module: usize,
    pub id: usize,
}
#[derive(Clone, Debug, Serialize)]
pub struct StructDef {
    pub name: String,
    pub name_span: Span,
    pub public: bool,
    pub fields: Vec<Param>,
    pub span: Span,
    pub module: usize,
    pub id: usize,
}
#[derive(Clone, Debug, Serialize)]
pub struct Import {
    pub path: String,
    pub alias: String,
    pub span: Span,
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct Ast {
    pub functions: Vec<Function>,
    pub structs: Vec<StructDef>,
    pub imports: Vec<Import>,
}

pub fn parse(tokens: &[Token], file: usize, end: usize) -> Result<Ast> {
    let tokens = tokens
        .iter()
        .filter(|t| !t.kind.trivia())
        .cloned()
        .collect();
    Parser {
        tokens,
        pos: 0,
        eof: Span {
            file,
            start: end,
            end,
        },
        depth: 0,
        allow_empty_struct: true,
    }
    .program()
}
struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    eof: Span,
    depth: usize,
    allow_empty_struct: bool,
}
impl Parser {
    fn previous_span(&self) -> Span {
        self.tokens
            .get(self.pos.saturating_sub(1))
            .map(|t| t.span)
            .unwrap_or(self.eof)
    }
    fn span(&self) -> Span {
        self.tokens
            .get(self.pos)
            .map(|t| t.span)
            .unwrap_or(self.eof)
    }
    fn at(&self, k: &Kind) -> bool {
        self.tokens
            .get(self.pos)
            .is_some_and(|t| std::mem::discriminant(&t.kind) == std::mem::discriminant(k))
    }
    fn take(&mut self, k: &Kind) -> bool {
        if self.at(k) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn expect(&mut self, k: &Kind) -> Result<Span> {
        let span = self.span();
        if self.take(k) {
            return Ok(span);
        }
        let expected = match k {
            Kind::LBrace => "'{'",
            Kind::RBrace => "'}'",
            Kind::LParen => "'('",
            Kind::RParen => "')'",
            Kind::LBracket => "'['",
            Kind::RBracket => "']'",
            Kind::Colon => "':'",
            Kind::End => "'~'",
            Kind::Assign => "'='",
            Kind::As => "AS",
            _ => "token",
        };
        let found = self
            .tokens
            .get(self.pos)
            .map(|t| format!("{:?}", t.text.chars().take(80).collect::<String>()))
            .unwrap_or_else(|| "end of input".into());
        Err(self.error(format!("expected {expected}, found {found}")))
    }
    fn error(&self, msg: impl Into<String>) -> Diagnostic {
        Diagnostic::error("E0100", msg, self.span())
    }
    fn ident(&mut self) -> Result<(String, Span)> {
        let t = self
            .tokens
            .get(self.pos)
            .cloned()
            .ok_or_else(|| self.error("expected identifier"))?;
        if let Kind::Ident(s) = t.kind {
            self.pos += 1;
            Ok((s, t.span))
        } else {
            Err(self.error("expected identifier"))
        }
    }
    fn path(&mut self) -> Result<Vec<String>> {
        let mut p = vec![self.ident()?.0];
        while self.take(&Kind::Scope) {
            p.push(self.ident()?.0);
        }
        Ok(p)
    }
    fn ty(&mut self) -> Result<Type> {
        self.depth += 1;
        if self.depth > 128 {
            return Err(self.error("type nesting limit exceeded"));
        }
        let t = if self.take(&Kind::IntType) {
            Type::Int
        } else if self.take(&Kind::FloatType) {
            Type::Float
        } else if self.take(&Kind::BoolType) {
            Type::Bool
        } else if self.take(&Kind::StrType) {
            Type::Str
        } else if self.take(&Kind::UnitType) {
            Type::Unit
        } else if self.take(&Kind::LBracket) {
            let t = self.ty()?;
            self.expect(&Kind::RBracket)?;
            Type::Array(Box::new(t))
        } else {
            Type::Named(self.path()?)
        };
        self.depth -= 1;
        Ok(t)
    }
    fn program(&mut self) -> Result<Ast> {
        let mut ast = Ast::default();
        while self.pos < self.tokens.len() {
            let start = self.span();
            let public = self.take(&Kind::Pub);
            if self.take(&Kind::Import) {
                if public {
                    return Err(self.error("imports cannot be PUB"));
                }
                let t = self
                    .tokens
                    .get(self.pos)
                    .cloned()
                    .ok_or_else(|| self.error("expected import path"))?;
                let Kind::Str(raw) = t.kind else {
                    return Err(self.error("expected quoted import path"));
                };
                self.pos += 1;
                let path = decode(&raw, t.span)?;
                self.expect(&Kind::As)?;
                let alias = self.ident()?.0;
                let end = self.expect(&Kind::End)?;
                ast.imports.push(Import {
                    path,
                    alias,
                    span: start.join(end),
                });
            } else if self.take(&Kind::Fn) {
                let (name, name_span) = self.ident()?;
                self.expect(&Kind::LParen)?;
                let mut params = Vec::new();
                while !self.at(&Kind::RParen) {
                    let (name, span) = self.ident()?;
                    self.expect(&Kind::Colon)?;
                    let ty = self.ty()?;
                    params.push(Param {
                        name,
                        ty,
                        span,
                        id: usize::MAX,
                    });
                    if !self.take(&Kind::Comma) {
                        break;
                    }
                }
                self.expect(&Kind::RParen)?;
                let result = if self.take(&Kind::Colon) {
                    self.ty()?
                } else {
                    Type::Unit
                };
                let body = self.block()?;
                ast.functions.push(Function {
                    name,
                    name_span,
                    public,
                    params,
                    result,
                    body,
                    span: start.join(self.previous_span()),
                    module: 0,
                    id: usize::MAX,
                });
            } else if self.take(&Kind::Struct) {
                let (name, name_span) = self.ident()?;
                self.expect(&Kind::LBrace)?;
                let mut fields = Vec::new();
                while !self.at(&Kind::RBrace) {
                    let (name, span) = self.ident()?;
                    self.expect(&Kind::Colon)?;
                    let ty = self.ty()?;
                    fields.push(Param {
                        name,
                        ty,
                        span,
                        id: usize::MAX,
                    });
                    if !self.take(&Kind::Comma) {
                        break;
                    }
                }
                self.expect(&Kind::RBrace)?;
                ast.structs.push(StructDef {
                    name,
                    name_span,
                    public,
                    fields,
                    span: start.join(self.previous_span()),
                    module: 0,
                    id: usize::MAX,
                });
            } else {
                return Err(self.error("expected FN, ST or IM at module scope"));
            }
        }
        Ok(ast)
    }
    fn block(&mut self) -> Result<Vec<Stmt>> {
        self.depth += 1;
        if self.depth > 128 {
            return Err(self.error("block nesting limit exceeded"));
        }
        self.expect(&Kind::LBrace)?;
        let mut body = Vec::new();
        while !self.at(&Kind::RBrace) {
            if self.pos >= self.tokens.len() {
                return Err(self.error("expected closing brace"));
            }
            body.push(self.stmt()?);
        }
        self.expect(&Kind::RBrace)?;
        self.depth -= 1;
        Ok(body)
    }
    fn stmt(&mut self) -> Result<Stmt> {
        let span = self.span();
        let kind = if self.take(&Kind::Let) {
            let (name, name_span) = self.ident()?;
            let annotation = if self.take(&Kind::Colon) {
                Some(self.ty()?)
            } else {
                None
            };
            self.expect(&Kind::Assign)?;
            let value = self.expr(0)?;
            self.expect(&Kind::End)?;
            StmtKind::Let {
                name,
                annotation,
                value,
                id: usize::MAX,
                name_span,
            }
        } else if self.take(&Kind::Ret) {
            let value = if self.at(&Kind::End) {
                None
            } else {
                Some(self.expr(0)?)
            };
            self.expect(&Kind::End)?;
            StmtKind::Ret(value)
        } else if self.take(&Kind::If) {
            let allow = self.allow_empty_struct;
            self.allow_empty_struct = false;
            let condition = self.expr(0)?;
            self.allow_empty_struct = allow;
            let yes = self.block()?;
            let no = if self.take(&Kind::Else) {
                Some(self.block()?)
            } else {
                None
            };
            StmtKind::If { condition, yes, no }
        } else if self.take(&Kind::While) {
            let allow = self.allow_empty_struct;
            self.allow_empty_struct = false;
            let condition = self.expr(0)?;
            self.allow_empty_struct = allow;
            let body = self.block()?;
            StmtKind::While { condition, body }
        } else if self.take(&Kind::Break) || self.take(&Kind::Brk) {
            self.expect(&Kind::End)?;
            StmtKind::Break
        } else if self.take(&Kind::Continue) || self.take(&Kind::Cont) {
            self.expect(&Kind::End)?;
            StmtKind::Continue
        } else if self.at(&Kind::InputInt)
            || self.at(&Kind::InputFloat)
            || self.at(&Kind::InputString)
        {
            let k = self.tokens[self.pos].kind.clone();
            self.pos += 1;
            let (name, s) = self.ident()?;
            self.expect(&Kind::End)?;
            let builtin = match k {
                Kind::InputInt => "read_int",
                Kind::InputFloat => "read_float",
                _ => "read_line",
            };
            let value = Expr::new(
                ExprKind::Call {
                    name: vec![builtin.into()],
                    args: vec![],
                    target: None,
                },
                span,
            );
            StmtKind::Assign {
                place: Expr::new(
                    ExprKind::Var {
                        name,
                        id: usize::MAX,
                    },
                    s,
                ),
                value,
            }
        } else if self.at(&Kind::OutputInt)
            || self.at(&Kind::OutputFloat)
            || self.at(&Kind::OutputString)
        {
            let k = self.tokens[self.pos].kind.clone();
            self.pos += 1;
            let value = self.expr(0)?;
            self.expect(&Kind::End)?;
            let name = match k {
                Kind::OutputInt => "output_int",
                Kind::OutputFloat => "output_float",
                _ => "output_str",
            };
            StmtKind::Expr(Expr::new(
                ExprKind::Call {
                    name: vec![name.into()],
                    args: vec![value],
                    target: None,
                },
                span,
            ))
        } else {
            let place = self.expr(0)?;
            if self.take(&Kind::Assign) {
                let value = self.expr(0)?;
                self.expect(&Kind::End)?;
                StmtKind::Assign { place, value }
            } else {
                self.expect(&Kind::End)?;
                StmtKind::Expr(place)
            }
        };
        Ok(Stmt {
            kind,
            span: span.join(self.previous_span()),
            terminates: false,
        })
    }
    fn expr(&mut self, min: u8) -> Result<Expr> {
        self.depth += 1;
        if self.depth > 128 {
            return Err(self.error("expression nesting limit exceeded"));
        }
        let mut lhs = self.atom()?;
        let mut operations = 0;
        loop {
            operations += 1;
            if operations > 256 {
                return Err(self.error("expression complexity limit exceeded"));
            }
            if self.take(&Kind::LBracket) {
                let idx = self.expr(0)?;
                let end = self.expect(&Kind::RBracket)?;
                let span = lhs.span.join(end);
                lhs = Expr::new(ExprKind::Index(Box::new(lhs), Box::new(idx)), span);
                continue;
            }
            if self.take(&Kind::Dot) {
                let (name, end) = self.ident()?;
                let span = lhs.span.join(end);
                lhs = Expr::new(
                    ExprKind::Field {
                        base: Box::new(lhs),
                        name,
                        index: usize::MAX,
                    },
                    span,
                );
                continue;
            }
            let op = match self.tokens.get(self.pos).map(|t| &t.kind) {
                Some(Kind::Plus) => Op::Add,
                Some(Kind::Minus) => Op::Sub,
                Some(Kind::Star) => Op::Mul,
                Some(Kind::Slash) => Op::Div,
                Some(Kind::Percent) => Op::Mod,
                Some(Kind::Eq) => Op::Eq,
                Some(Kind::Ne) => Op::Ne,
                Some(Kind::Lt) => Op::Lt,
                Some(Kind::Le) => Op::Le,
                Some(Kind::Gt) => Op::Gt,
                Some(Kind::Ge) => Op::Ge,
                Some(Kind::And) => Op::And,
                Some(Kind::Or) => Op::Or,
                _ => break,
            };
            let prec = match op {
                Op::Or => 1,
                Op::And => 2,
                Op::Eq | Op::Ne => 3,
                Op::Lt | Op::Le | Op::Gt | Op::Ge => 4,
                Op::Add | Op::Sub => 5,
                _ => 6,
            };
            if prec < min {
                break;
            }
            self.pos += 1;
            let rhs = self.expr(prec + 1)?;
            if op.comparison()
                && [&lhs, &rhs]
                    .iter()
                    .any(|e| matches!(&e.kind,ExprKind::Binary(o,_,_) if o.comparison()))
            {
                return Err(Diagnostic::error(
                    "E0101",
                    "comparison chains require explicit parentheses",
                    lhs.span.join(rhs.span),
                ));
            }
            let span = lhs.span.join(rhs.span);
            lhs = Expr::new(ExprKind::Binary(op, Box::new(lhs), Box::new(rhs)), span);
        }
        self.depth -= 1;
        if lhs.height > 128 {
            return Err(self.error("expression tree nesting limit exceeded"));
        }
        Ok(lhs)
    }
    fn atom(&mut self) -> Result<Expr> {
        let token = self
            .tokens
            .get(self.pos)
            .cloned()
            .ok_or_else(|| self.error("expected expression"))?;
        let span = token.span;
        self.pos += 1;
        let kind = match token.kind {
            Kind::Int(s) => ExprKind::Int(s.parse().map_err(|_| {
                Diagnostic::error("E0102", "integer literal is outside int range", span)
            })?),
            Kind::Float(s) => {
                let f: f64 = s.parse().map_err(|_| self.error("invalid float literal"))?;
                if !f.is_finite() {
                    return Err(Diagnostic::error(
                        "E0102",
                        "float literal is not finite",
                        span,
                    ));
                }
                ExprKind::Float(f)
            }
            Kind::Str(s) => ExprKind::Str(decode(&s, span)?),
            Kind::True => ExprKind::Bool(true),
            Kind::False => ExprKind::Bool(false),
            Kind::Minus => {
                if let Some(Token {
                    kind: Kind::Int(s),
                    span: end,
                    ..
                }) = self.tokens.get(self.pos)
                    && s.parse::<u64>() == Ok(1_u64 << 63)
                {
                    let end = *end;
                    self.pos += 1;
                    return Ok(Expr::new(ExprKind::Int(i64::MIN), span.join(end)));
                }
                ExprKind::Unary(Op::Neg, Box::new(self.expr(7)?))
            }
            Kind::Not => ExprKind::Unary(Op::Not, Box::new(self.expr(7)?)),
            Kind::LParen => {
                let allow = self.allow_empty_struct;
                self.allow_empty_struct = true;
                let e = self.expr(0)?;
                self.allow_empty_struct = allow;
                let end = self.expect(&Kind::RParen)?;
                return Ok(Expr::new(ExprKind::Group(Box::new(e)), span.join(end)));
            }
            Kind::LBracket => {
                let mut values = Vec::new();
                while !self.at(&Kind::RBracket) {
                    values.push(self.expr(0)?);
                    if !self.take(&Kind::Comma) {
                        break;
                    }
                }
                self.expect(&Kind::RBracket)?;
                ExprKind::Array(values)
            }
            Kind::Ident(s) => return self.named(vec![s], span),
            Kind::IntType => return self.named(vec!["int".into()], span),
            Kind::FloatType => return self.named(vec!["float".into()], span),
            Kind::StrType => return self.named(vec!["str".into()], span),
            _ => return Err(Diagnostic::error("E0100", "expected expression", span)),
        };
        let end = self
            .tokens
            .get(self.pos.saturating_sub(1))
            .map(|t| t.span)
            .unwrap_or(span);
        Ok(Expr::new(kind, span.join(end)))
    }
    fn named(&mut self, mut name: Vec<String>, span: Span) -> Result<Expr> {
        while self.take(&Kind::Scope) {
            name.push(self.ident()?.0);
        }
        let kind = if self.take(&Kind::LParen) {
            let allow = self.allow_empty_struct;
            self.allow_empty_struct = true;
            let mut args = Vec::new();
            while !self.at(&Kind::RParen) {
                args.push(self.expr(0)?);
                if !self.take(&Kind::Comma) {
                    break;
                }
            }
            self.expect(&Kind::RParen)?;
            self.allow_empty_struct = allow;
            ExprKind::Call {
                name,
                args,
                target: None,
            }
        } else if self.at(&Kind::LBrace)
            && ((self
                .tokens
                .get(self.pos + 1)
                .is_some_and(|t| matches!(t.kind, Kind::Ident(_)))
                && self
                    .tokens
                    .get(self.pos + 2)
                    .is_some_and(|t| t.kind == Kind::Colon))
                || (self.allow_empty_struct
                    && self
                        .tokens
                        .get(self.pos + 1)
                        .is_some_and(|t| t.kind == Kind::RBrace)))
        {
            self.pos += 1;
            let mut fields = Vec::new();
            while !self.at(&Kind::RBrace) {
                let key = self.ident()?.0;
                self.expect(&Kind::Colon)?;
                fields.push((key, self.expr(0)?));
                if !self.take(&Kind::Comma) {
                    break;
                }
            }
            self.expect(&Kind::RBrace)?;
            ExprKind::Struct {
                name,
                fields,
                id: usize::MAX,
            }
        } else {
            if name.len() != 1 {
                return Err(Diagnostic::error(
                    "E0100",
                    "qualified name must be called",
                    span,
                ));
            }
            ExprKind::Var {
                name: name.remove(0),
                id: usize::MAX,
            }
        };
        let end = self
            .tokens
            .get(self.pos.saturating_sub(1))
            .map(|t| t.span)
            .unwrap_or(span);
        Ok(Expr::new(kind, span.join(end)))
    }
}
fn decode(raw: &str, span: Span) -> Result<String> {
    let mut out = String::new();
    let mut chars = raw[1..raw.len() - 1].chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let escape = chars
            .next()
            .ok_or_else(|| Diagnostic::error("E0103", "unterminated escape", span))?;
        out.push(match escape {
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            '0' => '\0',
            '"' => '"',
            '\\' => '\\',
            'u' => {
                if chars.next() != Some('{') {
                    return Err(Diagnostic::error(
                        "E0103",
                        "use \\u{hex} for Unicode escapes",
                        span,
                    ));
                }
                let mut hex = String::new();
                let mut closed = false;
                for c in chars.by_ref() {
                    if c == '}' {
                        closed = true;
                        break;
                    }
                    hex.push(c);
                    if hex.len() > 6 {
                        break;
                    }
                }
                if !closed {
                    return Err(Diagnostic::error("E0103", "invalid Unicode escape", span));
                }
                u32::from_str_radix(&hex, 16)
                    .ok()
                    .and_then(char::from_u32)
                    .ok_or_else(|| Diagnostic::error("E0103", "invalid Unicode scalar", span))?
            }
            _ => {
                return Err(Diagnostic::error(
                    "E0103",
                    format!("unknown escape \\{escape}"),
                    span,
                ));
            }
        });
    }
    Ok(out)
}
