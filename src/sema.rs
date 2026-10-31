use crate::{
    diagnostics::{Diagnostic, Result},
    parser::*,
    source::{SourceFile, Span},
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Debug)]
pub enum ModuleRef {
    File(usize),
    Standard(String),
}
#[derive(Clone, Debug)]
pub struct Module {
    pub ast: Ast,
    pub imports: BTreeMap<String, ModuleRef>,
}
#[derive(Clone, Debug, Serialize)]
pub struct SymbolUse {
    pub name: String,
    pub span: Span,
    pub definition: Span,
    pub ty: Type,
}
#[derive(Clone, Debug, Serialize)]
pub struct CheckedProgram {
    pub(crate) sources: Vec<SourceFileSerde>,
    pub(crate) functions: Vec<Function>,
    pub(crate) structs: Vec<StructDef>,
    pub(crate) entry: Option<usize>,
    pub symbols: Vec<SymbolUse>,
}

impl CheckedProgram {
    pub fn type_name(&self, ty: &Type) -> String {
        match ty {
            Type::Struct(id) => self
                .structs
                .get(*id)
                .map(|s| s.name.clone())
                .unwrap_or_else(|| ty.to_string()),
            Type::Array(t) => format!("[{}]", self.type_name(t)),
            _ => ty.to_string(),
        }
    }
    pub fn symbol_description(&self, symbol: &SymbolUse) -> String {
        if let Some(f) = self
            .functions
            .iter()
            .find(|f| f.name_span == symbol.definition)
        {
            format!(
                "FN {}({}): {}",
                f.name,
                f.params
                    .iter()
                    .map(|p| format!("{}: {}", p.name, self.type_name(&p.ty)))
                    .collect::<Vec<_>>()
                    .join(", "),
                self.type_name(&f.result)
            )
        } else {
            format!("{}: {}", symbol.name, self.type_name(&symbol.ty))
        }
    }
}
// Source text is kept outside serialized IR; physical positions remain available to the backend.
#[derive(Clone, Debug)]
pub struct SourceFileSerde(pub SourceFile);
impl Serialize for SourceFileSerde {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        self.0.path.to_string_lossy().serialize(s)
    }
}
#[derive(Clone)]
struct Binding {
    id: usize,
    ty: Type,
    span: Span,
}
struct Checker {
    modules: Vec<Module>,
    functions: Vec<Function>,
    structs: Vec<StructDef>,
    names: BTreeMap<(usize, String), (bool, bool, usize)>,
    scopes: Vec<BTreeMap<String, Binding>>,
    symbols: Vec<SymbolUse>,
    next: usize,
    current: usize,
    result: Type,
    loops: usize,
}
pub(crate) fn check(modules: Vec<Module>, sources: Vec<SourceFile>) -> Result<CheckedProgram> {
    let mut c = Checker {
        modules,
        functions: vec![],
        structs: vec![],
        names: BTreeMap::new(),
        scopes: vec![],
        symbols: vec![],
        next: 0,
        current: 0,
        result: Type::Unit,
        loops: 0,
    };
    for (module, m) in c.modules.iter().enumerate() {
        for s in &m.ast.structs {
            let mut s = s.clone();
            s.module = module;
            s.id = c.structs.len();
            if c.names
                .insert((module, s.name.clone()), (true, s.public, s.id))
                .is_some()
            {
                return Err(Diagnostic::error(
                    "E0201",
                    "duplicate top-level name",
                    s.name_span,
                ));
            }
            c.structs.push(s);
        }
        for f in &m.ast.functions {
            let mut f = f.clone();
            f.module = module;
            f.id = c.functions.len();
            if c.names
                .insert((module, f.name.clone()), (false, f.public, f.id))
                .is_some()
            {
                return Err(Diagnostic::error(
                    "E0201",
                    "duplicate top-level name",
                    f.name_span,
                ));
            }
            c.functions.push(f);
        }
    }
    for i in 0..c.structs.len() {
        let mut s = c.structs[i].clone();
        let mut seen = BTreeSet::new();
        for p in &mut s.fields {
            if !seen.insert(p.name.clone()) {
                return Err(Diagnostic::error("E0201", "duplicate field", p.span));
            }
            p.ty = c.resolve_type(&p.ty, s.module, p.span)?;
            if p.ty == Type::Unit {
                return Err(Diagnostic::error(
                    "E0202",
                    "fields cannot have unit type",
                    p.span,
                ));
            }
        }
        c.structs[i] = s;
    }
    fn visit(
        id: usize,
        defs: &[StructDef],
        active: &mut BTreeSet<usize>,
        done: &mut BTreeSet<usize>,
    ) -> Result<()> {
        if active.len() > 128 {
            return Err(Diagnostic::error(
                "E0203",
                "struct nesting limit exceeded",
                defs[id].span,
            ));
        }
        if done.contains(&id) {
            return Ok(());
        }
        if !active.insert(id) {
            return Err(Diagnostic::error(
                "E0203",
                "recursive struct types are not supported",
                defs[id].span,
            ));
        }
        fn inner(
            t: &Type,
            defs: &[StructDef],
            a: &mut BTreeSet<usize>,
            d: &mut BTreeSet<usize>,
        ) -> Result<()> {
            match t {
                Type::Struct(id) => visit(*id, defs, a, d),
                Type::Array(t) => inner(t, defs, a, d),
                _ => Ok(()),
            }
        }
        for f in &defs[id].fields {
            inner(&f.ty, defs, active, done)?;
        }
        active.remove(&id);
        done.insert(id);
        Ok(())
    }
    for i in 0..c.structs.len() {
        visit(i, &c.structs, &mut BTreeSet::new(), &mut BTreeSet::new())?;
    }
    for i in 0..c.functions.len() {
        let mut f = c.functions[i].clone();
        for p in &mut f.params {
            p.ty = c.resolve_type(&p.ty, f.module, p.span)?;
            if p.ty == Type::Unit {
                return Err(Diagnostic::error(
                    "E0202",
                    "parameters cannot have unit type",
                    p.span,
                ));
            }
        }
        f.result = c.resolve_type(&f.result, f.module, f.span)?;
        c.functions[i] = f;
    }
    let entry = c
        .names
        .get(&(0, "main".into()))
        .filter(|v| !v.0)
        .map(|v| v.2);
    if let Some(entry) = entry
        && (c.functions[entry].result != Type::Int || !c.functions[entry].params.is_empty())
    {
        return Err(Diagnostic::error(
            "E0204",
            "main must have signature FN main(): int",
            c.functions[entry].span,
        ));
    }
    for f in &c.functions {
        c.symbols.push(SymbolUse {
            name: f.name.clone(),
            span: f.name_span,
            definition: f.name_span,
            ty: f.result.clone(),
        });
    }
    for i in 0..c.functions.len() {
        let mut f = c.functions[i].clone();
        c.current = f.module;
        c.result = f.result.clone();
        c.scopes = vec![BTreeMap::new()];
        c.loops = 0;
        for p in &mut f.params {
            p.id = c.bind(&p.name, p.ty.clone(), p.span)?;
        }
        let returns = c.block(&mut f.body, false)?;
        if f.result != Type::Unit && !returns {
            return Err(Diagnostic::error(
                "E0205",
                format!(
                    "function {} must return {} on every path",
                    f.name,
                    c.type_name(&f.result)
                ),
                f.span,
            ));
        }
        c.functions[i] = f;
    }
    Ok(CheckedProgram {
        sources: sources.into_iter().map(SourceFileSerde).collect(),
        functions: c.functions,
        structs: c.structs,
        entry,
        symbols: c.symbols,
    })
}
impl Checker {
    fn type_name(&self, ty: &Type) -> String {
        match ty {
            Type::Struct(id) => {
                let d = &self.structs[*id];
                let alias =
                    self.modules[self.current]
                        .imports
                        .iter()
                        .find_map(|(alias, m)| match m {
                            ModuleRef::File(module)
                                if *module == d.module && d.module != self.current =>
                            {
                                Some(alias)
                            }
                            _ => None,
                        });
                if let Some(alias) = alias {
                    format!("{alias}::{}", d.name)
                } else {
                    d.name.clone()
                }
            }
            Type::Array(t) => format!("[{}]", self.type_name(t)),
            _ => ty.to_string(),
        }
    }
    fn resolve_type(&self, t: &Type, module: usize, span: Span) -> Result<Type> {
        match t {
            Type::Named(p) => {
                let from = module;
                let (module, name) = self.qualify(p, module, span)?;
                let Some((true, public, id)) = self.names.get(&(module, name.clone())) else {
                    return Err(Diagnostic::error(
                        "E0206",
                        format!("unknown type {}", p.join("::")),
                        span,
                    ));
                };
                if module != from && !public {
                    return Err(Diagnostic::error("E0214", "type is private", span));
                }
                Ok(Type::Struct(*id))
            }
            Type::Array(t) => {
                let t = self.resolve_type(t, module, span)?;
                if t == Type::Unit {
                    return Err(Diagnostic::error(
                        "E0202",
                        "array elements cannot be unit",
                        span,
                    ));
                }
                Ok(Type::Array(Box::new(t)))
            }
            _ => Ok(t.clone()),
        }
    }
    fn qualify(&self, p: &[String], module: usize, span: Span) -> Result<(usize, String)> {
        if p.len() == 1 {
            return Ok((module, p[0].clone()));
        }
        if p.len() != 2 {
            return Err(Diagnostic::error("E0206", "expected alias::name", span));
        }
        match self.modules[module].imports.get(&p[0]) {
            Some(ModuleRef::File(id)) => Ok((*id, p[1].clone())),
            _ => Err(Diagnostic::error(
                "E0206",
                format!("unknown module alias {}", p[0]),
                span,
            )),
        }
    }
    fn bind(&mut self, name: &str, ty: Type, span: Span) -> Result<usize> {
        let scope = self.scopes.last_mut().expect("scope");
        if scope.contains_key(name) {
            return Err(Diagnostic::error(
                "E0201",
                format!("duplicate local {name}"),
                span,
            ));
        }
        let id = self.next;
        self.next += 1;
        scope.insert(
            name.into(),
            Binding {
                id,
                ty: ty.clone(),
                span,
            },
        );
        self.symbols.push(SymbolUse {
            name: name.into(),
            span,
            definition: span,
            ty,
        });
        Ok(id)
    }
    fn require(&self, actual: &Type, expected: &Type, span: Span) -> Result<()> {
        if actual == expected {
            Ok(())
        } else {
            Err(Diagnostic::error(
                "E0207",
                format!(
                    "expected {}, found {}",
                    self.type_name(expected),
                    self.type_name(actual)
                ),
                span,
            ))
        }
    }
    fn block(&mut self, body: &mut [Stmt], scope: bool) -> Result<bool> {
        if scope {
            self.scopes.push(BTreeMap::new());
        }
        let mut terminated = false;
        for s in body {
            if terminated {
                return Err(Diagnostic::error("E0208", "unreachable statement", s.span));
            }
            terminated = match &mut s.kind {
                StmtKind::Let {
                    name,
                    annotation,
                    value,
                    id,
                    name_span,
                } => {
                    let expected = annotation
                        .as_ref()
                        .map(|t| self.resolve_type(t, self.current, s.span))
                        .transpose()?;
                    let ty = self.expr(value, expected.as_ref())?;
                    if ty == Type::Unit {
                        return Err(Diagnostic::error(
                            "E0202",
                            "cannot bind a unit value",
                            value.span,
                        ));
                    }
                    if let Some(t) = expected {
                        self.require(&ty, &t, value.span)?;
                        *annotation = Some(t);
                    }
                    *id = self.bind(name, ty, *name_span)?;
                    false
                }
                StmtKind::Assign { place, value } => {
                    if !Self::place(place) {
                        return Err(Diagnostic::error(
                            "E0209",
                            "assignment target must be a local variable or its element/field",
                            place.span,
                        ));
                    }
                    let ty = self.expr(place, None)?;
                    let rhs = self.expr(value, Some(&ty))?;
                    self.require(&rhs, &ty, value.span)?;
                    false
                }
                StmtKind::Ret(e) => {
                    let expected = self.result.clone();
                    let ty = if let Some(e) = e {
                        self.expr(e, Some(&expected))?
                    } else {
                        Type::Unit
                    };
                    self.require(&ty, &expected, s.span)?;
                    true
                }
                StmtKind::If { condition, yes, no } => {
                    let ty = self.expr(condition, Some(&Type::Bool))?;
                    self.require(&ty, &Type::Bool, condition.span)?;
                    let a = self.block(yes, true)?;
                    let b = if let Some(no) = no {
                        self.block(no, true)?
                    } else {
                        false
                    };
                    a && b
                }
                StmtKind::While { condition, body } => {
                    let ty = self.expr(condition, Some(&Type::Bool))?;
                    self.require(&ty, &Type::Bool, condition.span)?;
                    self.loops += 1;
                    self.block(body, true)?;
                    self.loops -= 1;
                    constant_true(condition) && !has_break(body)
                }
                StmtKind::Break | StmtKind::Continue => {
                    if self.loops == 0 {
                        return Err(Diagnostic::error(
                            "E0210",
                            "break/continue outside a loop",
                            s.span,
                        ));
                    }
                    true
                }
                StmtKind::Expr(e) => {
                    self.expr(e, None)?;
                    false
                }
            };
            s.terminates = terminated;
        }
        if scope {
            self.scopes.pop();
        }
        Ok(terminated)
    }
    fn place(e: &Expr) -> bool {
        match &e.kind {
            ExprKind::Var { .. } => true,
            ExprKind::Index(b, _) | ExprKind::Field { base: b, .. } => Self::place(b),
            _ => false,
        }
    }
    fn expr(&mut self, e: &mut Expr, expected: Option<&Type>) -> Result<Type> {
        let span = e.span;
        let ty = match &mut e.kind {
            ExprKind::Int(_) => Type::Int,
            ExprKind::Float(_) => Type::Float,
            ExprKind::Bool(_) => Type::Bool,
            ExprKind::Str(_) => Type::Str,
            ExprKind::Var { name, id } => {
                let b = self
                    .scopes
                    .iter()
                    .rev()
                    .find_map(|s| s.get(name))
                    .cloned()
                    .ok_or_else(|| {
                        Diagnostic::error("E0211", format!("unknown variable {name}"), span)
                    })?;
                *id = b.id;
                self.symbols.push(SymbolUse {
                    name: name.clone(),
                    span,
                    definition: b.span,
                    ty: b.ty.clone(),
                });
                b.ty
            }
            ExprKind::Group(inner) => self.expr(inner, expected)?,
            ExprKind::Unary(op, inner) => {
                let t = self.expr(inner, None)?;
                match op {
                    Op::Neg if t == Type::Int || t == Type::Float => t,
                    Op::Not if t == Type::Bool => Type::Bool,
                    _ => {
                        return Err(Diagnostic::error(
                            "E0212",
                            format!("operator {op:?} does not accept {}", self.type_name(&t)),
                            span,
                        ));
                    }
                }
            }
            ExprKind::Binary(op, l, r) => {
                let a = self.expr(l, None)?;
                let b = self.expr(r, Some(&a))?;
                self.require(&b, &a, r.span)?;
                match op {
                    Op::And | Op::Or => {
                        self.require(&a, &Type::Bool, l.span)?;
                        Type::Bool
                    }
                    Op::Eq | Op::Ne
                        if matches!(a, Type::Int | Type::Float | Type::Bool | Type::Str) =>
                    {
                        Type::Bool
                    }
                    Op::Lt | Op::Le | Op::Gt | Op::Ge if matches!(a, Type::Int | Type::Float) => {
                        Type::Bool
                    }
                    Op::Add if a == Type::Str => Type::Str,
                    Op::Add | Op::Sub | Op::Mul | Op::Div
                        if matches!(a, Type::Int | Type::Float) =>
                    {
                        a
                    }
                    Op::Mod if a == Type::Int => Type::Int,
                    _ => {
                        return Err(Diagnostic::error(
                            "E0212",
                            format!("operator {op:?} does not accept {}", self.type_name(&a)),
                            span,
                        ));
                    }
                }
            }
            ExprKind::Call { name, args, target } => {
                if name.len() == 1 && self.scopes.iter().any(|scope| scope.contains_key(&name[0])) {
                    return Err(Diagnostic::error(
                        "E0221",
                        "local value is not callable",
                        span,
                    ));
                }
                let standard = if name.len() == 2 {
                    match self.modules[self.current].imports.get(&name[0]) {
                        Some(ModuleRef::Standard(m)) => Some((m.as_str(), name[1].as_str())),
                        _ => None,
                    }
                } else {
                    None
                };
                let local = if name.len() == 1 {
                    self.names.get(&(self.current, name[0].clone())).copied()
                } else {
                    None
                };
                if standard.is_some() || (name.len() == 1 && local.is_none()) {
                    let builtin = if let Some((module, n)) = standard {
                        if !standard_member(module, n) {
                            return Err(Diagnostic::error(
                                "E0213",
                                format!("unknown standard library function {}", name.join("::")),
                                span,
                            ));
                        }
                        n.to_owned()
                    } else {
                        name[0].clone()
                    };
                    let t = self.builtin(&builtin, args, span)?;
                    *target = Some(Target::Builtin(builtin));
                    t
                } else {
                    let (module, n) = self.qualify(name, self.current, span)?;
                    let Some((false, public, id)) = self.names.get(&(module, n)).copied() else {
                        return Err(Diagnostic::error(
                            "E0213",
                            format!("unknown function {}", name.join("::")),
                            span,
                        ));
                    };
                    let f = self.functions[id].clone();
                    if module != self.current && !public {
                        return Err(Diagnostic::error("E0214", "function is private", span));
                    }
                    if args.len() != f.params.len() {
                        return Err(Diagnostic::error(
                            "E0215",
                            format!(
                                "expected {} arguments, found {}",
                                f.params.len(),
                                args.len()
                            ),
                            span,
                        ));
                    }
                    for (a, p) in args.iter_mut().zip(&f.params) {
                        let t = self.expr(a, Some(&p.ty))?;
                        self.require(&t, &p.ty, a.span)?;
                    }
                    *target = Some(Target::Function(id));
                    let mut name_span = span;
                    name_span.start += name.join("::").len() - f.name.len();
                    name_span.end = name_span.start + f.name.len();
                    self.symbols.push(SymbolUse {
                        name: f.name.clone(),
                        span: name_span,
                        definition: f.name_span,
                        ty: f.result.clone(),
                    });
                    f.result
                }
            }
            ExprKind::Array(values) => {
                let mut item = match expected {
                    Some(Type::Array(t)) => Some((**t).clone()),
                    _ => None,
                };
                for value in values {
                    let t = self.expr(value, item.as_ref())?;
                    if t == Type::Unit {
                        return Err(Diagnostic::error(
                            "E0202",
                            "array elements cannot be unit",
                            value.span,
                        ));
                    }
                    if let Some(item) = &item {
                        self.require(&t, item, value.span)?;
                    } else {
                        item = Some(t);
                    }
                }
                Type::Array(Box::new(item.ok_or_else(|| {
                    Diagnostic::error("E0216", "empty array needs a type annotation", span)
                })?))
            }
            ExprKind::Index(base, index) => {
                let t = self.expr(base, None)?;
                let i = self.expr(index, Some(&Type::Int))?;
                self.require(&i, &Type::Int, index.span)?;
                let Type::Array(item) = t else {
                    return Err(Diagnostic::error(
                        "E0217",
                        "only arrays support indexing",
                        span,
                    ));
                };
                *item
            }
            ExprKind::Field { base, name, index } => {
                let t = self.expr(base, None)?;
                let Type::Struct(id) = t else {
                    return Err(Diagnostic::error(
                        "E0218",
                        "field access requires a struct",
                        span,
                    ));
                };
                let s = &self.structs[id];
                let Some((i, f)) = s.fields.iter().enumerate().find(|(_, f)| &f.name == name)
                else {
                    return Err(Diagnostic::error(
                        "E0218",
                        format!("unknown field {name}"),
                        span,
                    ));
                };
                *index = i;
                f.ty.clone()
            }
            ExprKind::Struct { name, fields, id } => {
                let t = self.resolve_type(&Type::Named(name.clone()), self.current, span)?;
                let Type::Struct(sid) = t else { unreachable!() };
                *id = sid;
                let definition = self.structs[sid].clone();
                if fields.len() != definition.fields.len() {
                    return Err(Diagnostic::error(
                        "E0219",
                        "struct literal must initialize every field exactly once",
                        span,
                    ));
                }
                let mut seen = BTreeSet::new();
                for (name, value) in fields.iter_mut() {
                    if !seen.insert(name.clone()) {
                        return Err(Diagnostic::error(
                            "E0219",
                            "duplicate field initializer",
                            value.span,
                        ));
                    }
                    let f = definition
                        .fields
                        .iter()
                        .find(|f| &f.name == name)
                        .ok_or_else(|| {
                            Diagnostic::error("E0218", format!("unknown field {name}"), value.span)
                        })?;
                    let t = self.expr(value, Some(&f.ty))?;
                    self.require(&t, &f.ty, value.span)?;
                }
                Type::Struct(sid)
            }
        };
        e.ty = ty.clone();
        Ok(ty)
    }
    fn builtin(&mut self, name: &str, args: &mut [Expr], span: Span) -> Result<Type> {
        let count = match name {
            "print" | "println" | "output_int" | "output_float" | "output_str" | "len"
            | "byte_len" | "int" | "float" | "str" | "sqrt" | "abs" | "assert" | "read_file" => 1,
            "push" | "pow" | "write_file" => 2,
            "read_int" | "read_float" | "read_line" | "args" => 0,
            _ => {
                return Err(Diagnostic::error(
                    "E0213",
                    format!("unknown function {name}"),
                    span,
                ));
            }
        };
        if args.len() != count {
            return Err(Diagnostic::error(
                "E0215",
                format!("{name} expects {count} arguments"),
                span,
            ));
        }
        let mut types = Vec::new();
        for a in args.iter_mut() {
            types.push(self.expr(a, None)?);
        }
        let t = types.first().cloned().unwrap_or(Type::Unit);
        match name {
            "print" | "println" => {
                if !matches!(t, Type::Int | Type::Float | Type::Bool | Type::Str) {
                    return Err(Diagnostic::error(
                        "E0220",
                        "print accepts int, float, bool or str",
                        span,
                    ));
                }
                Ok(Type::Unit)
            }
            "output_int" => {
                self.require(&t, &Type::Int, span)?;
                Ok(Type::Unit)
            }
            "output_float" => {
                self.require(&t, &Type::Float, span)?;
                Ok(Type::Unit)
            }
            "output_str" => {
                self.require(&t, &Type::Str, span)?;
                Ok(Type::Unit)
            }
            "read_int" => Ok(Type::Int),
            "read_float" => Ok(Type::Float),
            "read_line" => Ok(Type::Str),
            "args" => Ok(Type::Array(Box::new(Type::Str))),
            "len" => {
                if !matches!(t, Type::Str | Type::Array(_)) {
                    return Err(Diagnostic::error("E0220", "len accepts str or array", span));
                }
                Ok(Type::Int)
            }
            "byte_len" => {
                self.require(&t, &Type::Str, span)?;
                Ok(Type::Int)
            }
            "push" => {
                let Type::Array(item) = &t else {
                    return Err(Diagnostic::error("E0220", "push expects an array", span));
                };
                self.require(&types[1], item, span)?;
                Ok(t)
            }
            "int" | "float" => {
                if !matches!(t, Type::Int | Type::Float | Type::Str) {
                    return Err(Diagnostic::error(
                        "E0220",
                        "numeric conversion accepts int, float or str",
                        span,
                    ));
                }
                Ok(if name == "int" {
                    Type::Int
                } else {
                    Type::Float
                })
            }
            "str" => {
                if !matches!(t, Type::Int | Type::Float | Type::Bool | Type::Str) {
                    return Err(Diagnostic::error(
                        "E0220",
                        "str accepts a scalar value",
                        span,
                    ));
                }
                Ok(Type::Str)
            }
            "sqrt" => {
                self.require(&t, &Type::Float, span)?;
                Ok(Type::Float)
            }
            "pow" => {
                for t in &types {
                    self.require(t, &Type::Float, span)?;
                }
                Ok(Type::Float)
            }
            "abs" => {
                if t != Type::Int && t != Type::Float {
                    return Err(Diagnostic::error("E0220", "abs expects a number", span));
                }
                Ok(t)
            }
            "assert" => {
                self.require(&t, &Type::Bool, span)?;
                Ok(Type::Unit)
            }
            "read_file" => {
                self.require(&t, &Type::Str, span)?;
                Ok(Type::Str)
            }
            "write_file" => {
                for t in &types {
                    self.require(t, &Type::Str, span)?;
                }
                Ok(Type::Unit)
            }
            _ => unreachable!(),
        }
    }
}
fn standard_member(module: &str, name: &str) -> bool {
    match module {
        "std/io" => matches!(
            name,
            "print" | "println" | "read_int" | "read_float" | "read_line"
        ),
        "std/math" => matches!(name, "sqrt" | "pow" | "abs"),
        "std/str" => matches!(name, "len" | "byte_len" | "int" | "float"),
        "std/array" => matches!(name, "len" | "push"),
        "std/env" => name == "args",
        "std/fs" => matches!(name, "read_file" | "write_file"),
        _ => false,
    }
}

fn constant_true(expr: &Expr) -> bool {
    match &expr.kind {
        ExprKind::Bool(true) => true,
        ExprKind::Group(e) => constant_true(e),
        _ => false,
    }
}
fn has_break(stmts: &[Stmt]) -> bool {
    stmts.iter().any(|s| match &s.kind {
        StmtKind::Break => true,
        StmtKind::If { yes, no, .. } => {
            has_break(yes) || no.as_ref().is_some_and(|no| has_break(no))
        }
        _ => false,
    })
}
