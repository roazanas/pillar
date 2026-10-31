//! Pillar's reusable frontend, checked IR and native compiler.
//! User diagnostics are returned as data; only CLI adapters perform I/O reporting.
pub mod aot_backend;
pub mod compiler;
pub mod diagnostics;
pub mod formatter;
pub mod lexer;
pub mod lsp;
pub mod parser;
pub mod sema;
pub mod source;
pub mod transposer;
use diagnostics::Diagnostic;
use lexer::Token;
use parser::Ast;
use sema::{CheckedProgram, Module, ModuleRef};
use source::{SourceFile, Span};
use std::io::Read;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

pub struct Analysis {
    pub sources: Vec<SourceFile>,
    pub diagnostics: Vec<Diagnostic>,
    pub program: Option<CheckedProgram>,
    pub tokens: Vec<Vec<Token>>,
    pub ast: Vec<Ast>,
}
impl Analysis {
    pub fn success(&self) -> bool {
        self.program.is_some() && self.diagnostics.is_empty()
    }
}
pub fn read_source(path: &Path) -> std::io::Result<String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(1_048_577)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 1_048_576 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "source exceeds the 1 MiB limit",
        ));
    }
    String::from_utf8(bytes)
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "source must be UTF-8"))
}
pub fn analyze_path(path: &Path) -> Analysis {
    match read_source(path) {
        Ok(text) => analyze_source(SourceFile::vertical(0, path, text)),
        Err(e) => Analysis {
            sources: vec![SourceFile::horizontal(0, path, String::new())],
            diagnostics: vec![Diagnostic::error(
                "E0300",
                format!("cannot read source: {e}"),
                Span::default(),
            )],
            program: None,
            tokens: vec![],
            ast: vec![],
        },
    }
}
pub fn analyze_source(source: SourceFile) -> Analysis {
    analyze_with_overrides(source, &BTreeMap::new())
}
pub fn analyze_with_overrides(
    source: SourceFile,
    overrides: &BTreeMap<PathBuf, String>,
) -> Analysis {
    struct Loader<'a> {
        sources: Vec<SourceFile>,
        modules: Vec<Module>,
        tokens: Vec<Vec<Token>>,
        known: BTreeMap<PathBuf, usize>,
        active: Vec<PathBuf>,
        overrides: &'a BTreeMap<PathBuf, String>,
    }
    impl Loader<'_> {
        fn load(&mut self, mut source: SourceFile) -> diagnostics::Result<usize> {
            let path = std::fs::canonicalize(&source.path).unwrap_or_else(|_| source.path.clone());
            let id = self.sources.len();
            source.id = id;
            if id >= 128 {
                return Err(Diagnostic::error(
                    "E0301",
                    "module limit exceeded",
                    Span::default(),
                ));
            }
            self.known.insert(path.clone(), id);
            self.active.push(path);
            self.sources.push(source);
            self.modules.push(Module {
                ast: Ast::default(),
                imports: BTreeMap::new(),
            });
            self.tokens.push(Vec::new());
            let tokens = lexer::tokenize(&self.sources[id])?;
            let ast = parser::parse(&tokens, id, self.sources[id].logical.len())?;
            self.tokens[id] = tokens;
            let mut imports = BTreeMap::new();
            for import in &ast.imports {
                if imports.contains_key(&import.alias) {
                    return Err(Diagnostic::error(
                        "E0302",
                        "duplicate module alias",
                        import.span,
                    ));
                }
                let module = if import.path.starts_with("std/") {
                    if !matches!(
                        import.path.as_str(),
                        "std/io" | "std/math" | "std/str" | "std/array" | "std/env" | "std/fs"
                    ) {
                        return Err(Diagnostic::error(
                            "E0303",
                            "unknown standard module",
                            import.span,
                        ));
                    }
                    ModuleRef::Standard(import.path.clone())
                } else {
                    let p = self.sources[id]
                        .path
                        .parent()
                        .unwrap_or(Path::new("."))
                        .join(&import.path);
                    if p.extension().is_none_or(|x| x != "plr") {
                        return Err(Diagnostic::error(
                            "E0303",
                            "imports must name a .plr source",
                            import.span,
                        ));
                    }
                    let p = std::fs::canonicalize(&p).map_err(|e| {
                        Diagnostic::error(
                            "E0300",
                            format!("cannot import {}: {e}", p.display()),
                            import.span,
                        )
                    })?;
                    if self.active.contains(&p) {
                        return Err(Diagnostic::error(
                            "E0304",
                            format!("cyclic import: {}", p.display()),
                            import.span,
                        ));
                    }
                    let child = if let Some(id) = self.known.get(&p) {
                        *id
                    } else {
                        let text = if let Some(text) = self.overrides.get(&p) {
                            text.clone()
                        } else {
                            read_source(&p).map_err(|e| {
                                Diagnostic::error(
                                    "E0300",
                                    format!("cannot read import: {e}"),
                                    import.span,
                                )
                            })?
                        };
                        self.load(SourceFile::vertical(self.sources.len(), &p, text))?
                    };
                    ModuleRef::File(child)
                };
                imports.insert(import.alias.clone(), module);
            }
            self.modules[id] = Module { ast, imports };
            self.active.pop();
            Ok(id)
        }
    }
    let mut loader = Loader {
        sources: vec![],
        modules: vec![],
        tokens: vec![],
        known: BTreeMap::new(),
        active: vec![],
        overrides,
    };
    let loaded = loader.load(source);
    let ast = loader.modules.iter().map(|m| m.ast.clone()).collect();
    let mut diagnostics = vec![];
    let program = match loaded {
        Err(e) => {
            diagnostics.push(e);
            None
        }
        Ok(_) => match sema::check(loader.modules, loader.sources.clone()) {
            Ok(p) => Some(p),
            Err(e) => {
                diagnostics.push(e);
                None
            }
        },
    };
    Analysis {
        sources: loader.sources,
        diagnostics,
        program,
        tokens: loader.tokens,
        ast,
    }
}
