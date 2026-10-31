//! A dependency-light LSP stdio adapter. All coordinates on the wire are UTF-16.
use crate::{
    Analysis, analyze_with_overrides, formatter,
    lexer::{self, Kind},
    sema::SymbolUse,
    source::{SourceFile, Span},
    transposer,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::{self, BufRead, Write},
    path::{Path, PathBuf},
};
#[derive(Clone)]
struct Document {
    path: PathBuf,
    text: String,
    version: i64,
}
pub fn path_from_uri(uri: &str) -> Option<PathBuf> {
    let raw = uri.strip_prefix("file://")?;
    let raw = raw
        .strip_prefix("localhost/")
        .map(|s| format!("/{s}"))
        .unwrap_or_else(|| raw.to_owned());
    if !raw.starts_with('/') {
        return None;
    }
    let mut bytes = Vec::new();
    let mut iter = raw.as_bytes().iter().copied();
    while let Some(b) = iter.next() {
        if b == b'%' {
            let a = iter.next()?;
            let c = iter.next()?;
            bytes.push((a as char).to_digit(16)? as u8 * 16 + (c as char).to_digit(16)? as u8);
        } else {
            bytes.push(b);
        }
    }
    let text = String::from_utf8(bytes).ok()?;
    #[cfg(windows)]
    let text = text
        .strip_prefix('/')
        .filter(|s| s.as_bytes().get(1) == Some(&b':'))
        .unwrap_or(&text)
        .to_owned();
    Some(PathBuf::from(text))
}
pub fn uri_from_path(path: &Path) -> String {
    let absolute = std::fs::canonicalize(path).unwrap_or_else(|_| {
        if path.is_absolute() {
            path.to_owned()
        } else {
            std::env::current_dir().unwrap_or_default().join(path)
        }
    });
    let text = absolute.to_string_lossy().replace('\\', "/");
    let text = text.strip_prefix("//?/").unwrap_or(&text);
    let mut out = if text.starts_with('/') {
        "file://".to_owned()
    } else {
        "file:///".to_owned()
    };
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || b"/-_.~:".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}
fn position(source: &SourceFile, byte: usize) -> Value {
    let p = source.position(byte);
    let start = source.text[..p.byte]
        .rfind('\n')
        .map(|n| n + 1)
        .unwrap_or(0);
    json!({"line":p.line-1,"character":source.text[start..p.byte].encode_utf16().count()})
}
fn range(source: &SourceFile, span: Span) -> Value {
    let segments = source.segments(span);
    let first = &segments[0];
    json!({"start":position(source,first.start),"end":position(source,first.end)})
}
fn full_range(source: &SourceFile) -> Value {
    json!({"start":{"line":0,"character":0},"end":position(source,source.text.len())})
}
fn byte_at(source: &SourceFile, p: &Value) -> Option<usize> {
    let line = p["line"].as_u64()? as usize;
    let character = p["character"].as_u64()? as usize;
    let mut offset = 0;
    for (i, row) in source.text.split_inclusive('\n').enumerate() {
        if i == line {
            let mut count = 0;
            for (n, c) in row.char_indices() {
                if count == character {
                    return Some(offset + n);
                }
                count += c.len_utf16();
                if count > character {
                    return None;
                }
            }
            return (count == character).then_some(offset + row.len());
        }
        offset += row.len();
    }
    if line == source.text.lines().count() && character == 0 {
        Some(source.text.len())
    } else {
        None
    }
}
fn send(out: &mut impl Write, value: Value) -> io::Result<()> {
    let bytes = serde_json::to_vec(&value)?;
    write!(out, "Content-Length: {}\r\n\r\n", bytes.len())?;
    out.write_all(&bytes)?;
    out.flush()
}
fn analyze(doc: &Document, documents: &BTreeMap<String, Document>) -> Analysis {
    let overrides = documents
        .values()
        .map(|d| {
            (
                std::fs::canonicalize(&d.path).unwrap_or_else(|_| d.path.clone()),
                d.text.clone(),
            )
        })
        .collect();
    analyze_with_overrides(
        SourceFile::vertical(0, &doc.path, doc.text.clone()),
        &overrides,
    )
}
fn symbol<'a>(a: &'a Analysis, point: &Value) -> Option<&'a SymbolUse> {
    let source = a.sources.first()?;
    let byte = byte_at(source, point)?;
    a.program
        .as_ref()?
        .symbols
        .iter()
        .find(|s| s.span.file == 0 && source.segments(s.span).iter().any(|r| r.contains(&byte)))
}
fn diagnostics(
    out: &mut impl Write,
    uri: &str,
    doc: &Document,
    documents: &BTreeMap<String, Document>,
) -> io::Result<()> {
    let a = analyze(doc, documents);
    let ds=a.diagnostics.iter().map(|d|{let source=&a.sources[d.span.file];let label=if d.span.file==0{range(source,d.span)}else{json!({"start":{"line":0,"character":0},"end":{"line":0,"character":0}})};json!({"range":label,"severity":1,"code":d.code,"source":"pillar","message":if d.span.file==0{d.message.clone()}else{format!("{}: {}",source.path.display(),d.message)}})}).collect::<Vec<_>>();
    send(
        out,
        json!({"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{"uri":uri,"version":doc.version,"diagnostics":ds}}),
    )
}
pub fn serve(mut input: impl BufRead, mut output: impl Write) -> io::Result<()> {
    let mut documents = BTreeMap::<String, Document>::new();
    let mut shutdown = false;
    loop {
        let mut length = None;
        let mut header = String::new();
        let mut total = 0;
        loop {
            header.clear();
            if input.read_line(&mut header)? == 0 {
                return Ok(());
            }
            total += header.len();
            if total > 8192 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "LSP headers too large",
                ));
            }
            if header == "\r\n" || header == "\n" {
                break;
            }
            if let Some((name, value)) = header.split_once(':')
                && name.eq_ignore_ascii_case("Content-Length")
            {
                length = value.trim().parse::<usize>().ok();
            }
        }
        let length = length.filter(|n| *n <= 4_194_304).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "invalid LSP Content-Length")
        })?;
        let mut payload = vec![0; length];
        input.read_exact(&mut payload)?;
        let message: Value = match serde_json::from_slice(&payload) {
            Ok(m) => m,
            Err(_) => {
                send(
                    &mut output,
                    json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"invalid JSON"}}),
                )?;
                continue;
            }
        };
        let id = message.get("id").cloned();
        let method = message["method"].as_str().unwrap_or("");
        let params = &message["params"];
        if method == "exit" {
            return if shutdown {
                Ok(())
            } else {
                Err(io::Error::other("LSP exited before shutdown"))
            };
        }
        if method == "shutdown" {
            shutdown = true;
            if let Some(id) = id {
                send(&mut output, json!({"jsonrpc":"2.0","id":id,"result":null}))?;
            }
            continue;
        }
        if shutdown {
            if let Some(id) = id {
                send(
                    &mut output,
                    json!({"jsonrpc":"2.0","id":id,"error":{"code":-32600,"message":"server has shut down"}}),
                )?;
            }
            continue;
        }
        if method == "initialize" {
            if let Some(id) = id {
                send(
                    &mut output,
                    json!({"jsonrpc":"2.0","id":id,"result":{"serverInfo":{"name":"pillar","version":env!("CARGO_PKG_VERSION")},"capabilities":{"positionEncoding":"utf-16","textDocumentSync":1,"hoverProvider":true,"definitionProvider":true,"renameProvider":{"prepareProvider":true},"documentFormattingProvider":true,"completionProvider":{},"semanticTokensProvider":{"legend":{"tokenTypes":["keyword","variable","function","type","number","string","comment","operator"],"tokenModifiers":[]},"full":true}}}}),
                )?;
            }
            continue;
        }
        if method == "textDocument/didOpen" || method == "textDocument/didChange" {
            let td = &params["textDocument"];
            if let (Some(uri), Some(version)) = (td["uri"].as_str(), td["version"].as_i64()) {
                let text = if method.ends_with("didOpen") {
                    td["text"].as_str()
                } else {
                    params["contentChanges"]
                        .as_array()
                        .and_then(|a| a.last())
                        .and_then(|v| v["text"].as_str())
                };
                if let (Some(text), Some(path)) = (text, path_from_uri(uri))
                    && documents.get(uri).is_none_or(|d| version > d.version)
                {
                    documents.insert(
                        uri.into(),
                        Document {
                            path,
                            text: text.into(),
                            version,
                        },
                    );
                    let open = documents
                        .iter()
                        .map(|(uri, d)| (uri.clone(), d.clone()))
                        .collect::<Vec<_>>();
                    for (uri, doc) in open {
                        diagnostics(&mut output, &uri, &doc, &documents)?;
                    }
                }
            }
            continue;
        }
        if method == "textDocument/didClose" {
            if let Some(uri) = params["textDocument"]["uri"].as_str() {
                documents.remove(uri);
                send(
                    &mut output,
                    json!({"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{"uri":uri,"diagnostics":[]}}),
                )?;
            }
            continue;
        }
        let Some(id) = id else {
            continue;
        };
        let uri = params["textDocument"]["uri"].as_str().unwrap_or("");
        let doc = documents.get(uri);
        let result = if let Some(doc) = doc {
            let analysis = analyze(doc, &documents);
            request(method, params, &analysis, &documents)
        } else {
            Ok(Value::Null)
        };
        let response = match result {
            Ok(result) => json!({"jsonrpc":"2.0","id":id,"result":result}),
            Err((code, message)) => {
                json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
            }
        };
        send(&mut output, response)?;
    }
}
type RequestResult = std::result::Result<Value, (i64, &'static str)>;
fn request(
    method: &str,
    params: &Value,
    a: &Analysis,
    documents: &BTreeMap<String, Document>,
) -> RequestResult {
    let source = &a.sources[0];
    match method {
        "textDocument/hover" => Ok(symbol(a,&params["position"]).map(|s|json!({"contents":{"kind":"plaintext","value":a.program.as_ref().expect("resolved program").symbol_description(s)},"range":range(source,s.span)})).unwrap_or(Value::Null)),
        "textDocument/definition" => Ok(symbol(a,&params["position"]).map(|s| {
            let target=&a.sources[s.definition.file];
            json!({"uri":uri_from_path(&target.path),"range":range(target,s.definition)})
        }).unwrap_or(Value::Null)),
        "textDocument/formatting" => Ok(match formatter::vertical(source) {
            Ok(text) => json!([{"range":full_range(source),"newText":text}]),
            Err(_) => json!([]),
        }),
        "textDocument/completion" => Ok(completions(a)),
        "textDocument/prepareRename" => Ok(symbol(a,&params["position"]).map(|s|json!({"range":range(source,s.span),"placeholder":s.name})).unwrap_or(Value::Null)),
        "textDocument/rename" => rename(params,a,documents),
        "textDocument/semanticTokens/full" => Ok(semantic_tokens(source)),
        _ => Err((-32601,"method not found")),
    }
}
fn completions(a: &Analysis) -> Value {
    let mut items = [
        "FN",
        "LT",
        "RET",
        "IF",
        "EL",
        "WH",
        "IM",
        "AS",
        "PUB",
        "ST",
        "break",
        "continue",
        "true",
        "false",
        "int",
        "float",
        "bool",
        "str",
        "unit",
        "print",
        "println",
        "len",
        "push",
        "read_line",
        "read_int",
        "read_float",
        "args",
    ]
    .into_iter()
    .map(|name| json!({"label":name}))
    .collect::<Vec<_>>();
    if let Some(program) = &a.program {
        for function in &program.functions {
            if function.module == 0 {
                items.push(json!({"label":function.name,"kind":3}));
            }
        }
    }
    json!(items)
}
fn rename(params: &Value, a: &Analysis, documents: &BTreeMap<String, Document>) -> RequestResult {
    let new = params["newName"].as_str().unwrap_or("");
    if !valid_name(new) {
        return Err((-32602, "newName must be a non-keyword ASCII identifier"));
    }
    let Some(found) = symbol(a, &params["position"]) else {
        return Ok(Value::Null);
    };
    let program = a.program.as_ref().expect("resolved symbol");
    if program
        .symbols
        .iter()
        .any(|s| s.name == new && s.definition != found.definition)
    {
        return Err((
            -32602,
            "rename could capture an existing name; choose a different identifier",
        ));
    }
    let mut changes = serde_json::Map::new();
    let mut changed = BTreeMap::<PathBuf, String>::new();
    for source in &a.sources {
        let mut spans = program
            .symbols
            .iter()
            .filter(|s| s.definition == found.definition && s.span.file == source.id)
            .map(|s| s.span)
            .collect::<Vec<_>>();
        spans.sort_by_key(|span| std::cmp::Reverse(span.start));
        spans.dedup();
        if spans.is_empty() {
            continue;
        }
        let mut logical = source.logical.clone();
        for span in spans {
            logical.replace_range(span.start..span.end, new);
        }
        let text = transposer::transpose(logical.lines().collect(), false)
            .into_iter()
            .map(|l| l.trim_end().to_owned())
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        changed.insert(
            std::fs::canonicalize(&source.path).unwrap_or_else(|_| source.path.clone()),
            text.clone(),
        );
        changes.insert(
            uri_from_path(&source.path),
            json!([{"range":full_range(source),"newText":text}]),
        );
    }
    let mut overrides = documents
        .values()
        .map(|d| {
            (
                std::fs::canonicalize(&d.path).unwrap_or_else(|_| d.path.clone()),
                d.text.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    overrides.extend(changed.clone());
    let source = &a.sources[0];
    let root_path = std::fs::canonicalize(&source.path).unwrap_or_else(|_| source.path.clone());
    let root = changed
        .get(&root_path)
        .cloned()
        .unwrap_or_else(|| source.text.clone());
    let trial = analyze_with_overrides(SourceFile::vertical(0, &source.path, root), &overrides);
    if !trial.success() {
        return Err((-32602, "rename would introduce a name or type error"));
    }
    Ok(json!({"changes":changes}))
}
fn semantic_tokens(source: &SourceFile) -> Value {
    let mut physical = Vec::new();
    if let Ok(tokens) = lexer::tokenize(source) {
        for token in tokens {
            if token.kind == Kind::Space {
                continue;
            }
            let kind = match token.kind {
                Kind::Ident(_) => 1,
                Kind::IntType
                | Kind::FloatType
                | Kind::BoolType
                | Kind::StrType
                | Kind::UnitType => 3,
                Kind::Int(_) | Kind::Float(_) => 4,
                Kind::Str(_) => 5,
                Kind::LineComment | Kind::BlockComment => 6,
                Kind::Plus
                | Kind::Minus
                | Kind::Star
                | Kind::Slash
                | Kind::Percent
                | Kind::Eq
                | Kind::Ne
                | Kind::Lt
                | Kind::Le
                | Kind::Gt
                | Kind::Ge
                | Kind::And
                | Kind::Or
                | Kind::Not
                | Kind::Assign => 7,
                _ => 0,
            };
            for segment in source.segments(token.span) {
                let pos = position(source, segment.start);
                let length = source.text[segment].encode_utf16().count();
                if length > 0 {
                    physical.push((
                        pos["line"].as_u64().unwrap_or(0),
                        pos["character"].as_u64().unwrap_or(0),
                        length as u64,
                        kind,
                    ));
                }
            }
        }
    }
    physical.sort();
    physical.dedup();
    let mut data = Vec::new();
    let (mut line, mut column) = (0, 0);
    for (l, c, length, kind) in physical {
        data.extend([
            l - line,
            if l == line { c - column } else { c },
            length,
            kind,
            0,
        ]);
        line = l;
        column = c;
    }
    json!({"data":data})
}
fn valid_name(name: &str) -> bool {
    let source = SourceFile::horizontal(0, "name", name.into());
    matches!(
        lexer::tokenize(&source).as_deref(),
        Ok([crate::lexer::Token {
            kind: Kind::Ident(_),
            ..
        }])
    )
}
