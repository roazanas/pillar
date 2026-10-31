use pillar::{source::SourceFile, transposer};
use serde_json::{Value, json};
use std::{io::Cursor, path::Path, process::Command};
fn vertical(s: &str) -> String {
    transposer::transpose(s.lines().collect(), true).join("\n") + "\n"
}
fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_plrc"))
}
#[test]
fn cli_commands_and_atomic_output() {
    let dir = tempfile::Builder::new()
        .prefix("pillar cli ")
        .tempdir()
        .unwrap();
    let source = dir.path().join("main.plr");
    std::fs::write(
        &source,
        vertical("FN main(): int { println(\"works\")~ RET 0~ }"),
    )
    .unwrap();
    let exe = dir.path().join("output.o");
    let output = cli()
        .args(["build"])
        .arg(&source)
        .arg("-o")
        .arg(&exe)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(Command::new(&exe).output().unwrap().stdout, b"works\n");
    let before = std::fs::read(&exe).unwrap();
    let output = cli()
        .arg("build")
        .arg(&source)
        .arg("-o")
        .arg(&exe)
        .arg("--linker")
        .arg(dir.path().join("missing-linker"))
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(std::fs::read(&exe).unwrap(), before);
    let output = cli()
        .arg("build")
        .arg(&source)
        .arg("-o")
        .arg(&source)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("E0501"));
    assert!(
        cli()
            .arg("check")
            .arg(&source)
            .env("PILLAR_LINKER", "nonexistent")
            .status()
            .unwrap()
            .success()
    );
    for emit in ["tokens", "ast", "hir", "ir"] {
        let output = cli()
            .arg("build")
            .arg(&source)
            .arg("--emit")
            .arg(emit)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{emit}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!output.stdout.is_empty());
    }
    for shell in ["bash", "zsh", "fish", "powershell"] {
        let output = cli().args(["completions", shell]).output().unwrap();
        assert!(output.status.success());
        assert!(!output.stdout.is_empty());
    }
}
#[test]
fn legacy_transpose_format_and_json() {
    let dir = tempfile::tempdir().unwrap();
    let h = dir.path().join("main.rplr");
    let p = dir.path().join("main.plr");
    std::fs::write(&h, "FN main(): int { RET 0~ }\n").unwrap();
    let converted = cli().arg("-t").arg(&h).output().unwrap();
    assert!(converted.status.success());
    std::fs::write(&p, &converted.stdout).unwrap();
    let fmt = cli().arg("fmt").arg(&p).output().unwrap();
    assert!(
        fmt.status.success(),
        "{}",
        String::from_utf8_lossy(&fmt.stderr)
    );
    std::fs::write(&p, &fmt.stdout).unwrap();
    assert!(
        cli()
            .arg("fmt")
            .arg(&p)
            .arg("--check")
            .status()
            .unwrap()
            .success()
    );
    let exe = dir.path().join("old-out.exe");
    assert!(
        cli()
            .arg(&p)
            .arg("-o")
            .arg(&exe)
            .status()
            .unwrap()
            .success()
    );
    std::fs::write(&p, vertical("FN main(): int { OUTI missing~ RET 0~ }")).unwrap();
    let output = cli()
        .arg("check")
        .arg(&p)
        .args(["--diagnostic-format", "json"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let d: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(d["code"], "E0211");
    assert_eq!(d["labels"].as_array().unwrap().len(), 7);
    assert!(
        cli()
            .arg("fmt")
            .arg(&p)
            .arg("--write")
            .status()
            .unwrap()
            .success()
    ); // Syntax-valid files may be formatted despite a type error.
    std::fs::write(&p, vertical("FN main(): int { RET 0 }")).unwrap();
    let invalid = std::fs::read(&p).unwrap();
    assert!(
        !cli()
            .arg("fmt")
            .arg(&p)
            .arg("--write")
            .output()
            .unwrap()
            .status
            .success()
    );
    assert_eq!(std::fs::read(&p).unwrap(), invalid);
}
fn wire(messages: &[Value]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for message in messages {
        let payload = serde_json::to_vec(message).unwrap();
        bytes.extend(format!("Content-Length: {}\r\n\r\n", payload.len()).as_bytes());
        bytes.extend(payload);
    }
    bytes
}
fn responses(bytes: &[u8]) -> Vec<Value> {
    let mut at = 0;
    let mut out = vec![];
    while at < bytes.len() {
        let end = bytes[at..]
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .unwrap()
            + at;
        let header = std::str::from_utf8(&bytes[at..end]).unwrap();
        let len: usize = header.split(": ").nth(1).unwrap().parse().unwrap();
        at = end + 4;
        out.push(serde_json::from_slice(&bytes[at..at + len]).unwrap());
        at += len;
    }
    out
}
#[test]
fn lsp_framing_capabilities_positions_and_rename() {
    let text = vertical("FN main(): int {\n  LT x: int = 5~\n  OUTI x~\n  RET 0~\n}");
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("language server test.plr");
    let uri = pillar::lsp::uri_from_path(&path);
    assert_eq!(pillar::lsp::path_from_uri(&uri).unwrap(), path);
    let source = SourceFile::vertical(0, &path, text.clone());
    let index = source.logical.find("OUTI x").unwrap() + 5;
    let location = source.location(source.span(index..index + 1));
    let point = json!({"line":location.line-1,"character":location.column-1});
    let input = wire(&[
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":uri,"languageId":"pillar","version":1,"text":text}}}),
        json!({"jsonrpc":"2.0","id":2,"method":"textDocument/hover","params":{"textDocument":{"uri":uri},"position":point}}),
        json!({"jsonrpc":"2.0","id":3,"method":"textDocument/definition","params":{"textDocument":{"uri":uri},"position":point}}),
        json!({"jsonrpc":"2.0","id":4,"method":"textDocument/rename","params":{"textDocument":{"uri":uri},"position":point,"newName":"long_name"}}),
        json!({"jsonrpc":"2.0","id":5,"method":"textDocument/semanticTokens/full","params":{"textDocument":{"uri":uri}}}),
        json!({"jsonrpc":"2.0","id":7,"method":"textDocument/prepareRename","params":{"textDocument":{"uri":uri},"position":point}}),
        json!({"jsonrpc":"2.0","id":6,"method":"shutdown"}),
        json!({"jsonrpc":"2.0","method":"exit"}),
    ]);
    let mut out = Vec::new();
    pillar::lsp::serve(Cursor::new(input), &mut out).unwrap();
    let output = responses(&out);
    let get = |id| output.iter().find(|m| m["id"] == id).unwrap();
    assert_eq!(
        get(1)["result"]["capabilities"]["positionEncoding"],
        "utf-16"
    );
    assert_eq!(
        get(1)["result"]["capabilities"]["renameProvider"]["prepareProvider"],
        true
    );
    assert_eq!(get(7)["result"]["placeholder"], "x");
    assert_eq!(get(7)["result"]["range"], get(2)["result"]["range"]);
    assert!(
        get(2)["result"]["contents"]["value"]
            .as_str()
            .unwrap()
            .contains("x: int")
    );
    assert_eq!(get(3)["result"]["uri"], uri);
    let renamed = get(4)["result"]["changes"][&uri][0]["newText"]
        .as_str()
        .unwrap();
    let a = pillar::analyze_source(SourceFile::vertical(0, &path, renamed.into()));
    assert!(a.success(), "{:?}", a.diagnostics);
    assert!(a.sources[0].logical.contains("long_name"));
    assert!(!get(5)["result"]["data"].as_array().unwrap().is_empty());
}
#[test]
fn malformed_and_deep_input_never_panics() {
    let mut random = 17u64;
    let alphabet = b"FN LT RET IF EL WH (){}[]:~,=+-*/%\"\\0123abc\n\t@";
    for _ in 0..300 {
        let mut text = String::new();
        for _ in 0..80 {
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
            text.push(alphabet[(random >> 32) as usize % alphabet.len()] as char);
        }
        let result = std::panic::catch_unwind(|| {
            pillar::analyze_source(SourceFile::vertical(0, "fuzz.plr", text))
        });
        assert!(result.is_ok());
    }
    let text = format!(
        "FN main(): int {{ RET {}0{}~ }}",
        "(".repeat(500),
        ")".repeat(500)
    );
    assert!(!pillar::analyze_source(SourceFile::horizontal(0, "deep", text)).success());
    let text = format!("FN main(): int {{ RET {}0~ }}", "1 + ".repeat(2000));
    assert!(!pillar::analyze_source(SourceFile::horizontal(0, "deep", text)).success());
}

#[cfg(target_os = "linux")]
#[test]
fn non_unicode_filesystem_paths_do_not_panic() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let dir = tempfile::tempdir().unwrap();
    let path = dir
        .path()
        .join(OsString::from_vec(b"source-\xff.plr".to_vec()));
    std::fs::write(&path, vertical("FN main(): int { OUTI missing~ RET 0~ }")).unwrap();
    let output = cli()
        .arg("check")
        .arg(&path)
        .args(["--diagnostic-format", "json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let json: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(json["code"], "E0211");
    std::fs::write(&path, vertical("FN main(): int { RET 0~ }")).unwrap();
    let output = cli()
        .arg("build")
        .arg(&path)
        .args(["--emit", "hir"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(unix)]
#[test]
fn non_unicode_source_labels_are_serializable() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let path = std::path::PathBuf::from(OsString::from_vec(b"source-\xff.plr".to_vec()));
    let source = SourceFile::vertical(
        0,
        &path,
        vertical("FN main(): int { OUTI missing~ RET 0~ }"),
    );
    let a = pillar::analyze_source(source);
    let d = a.diagnostics[0].json(&a.sources);
    assert_eq!(d["code"], "E0211");
    let source = SourceFile::vertical(0, &path, vertical("FN main(): int { RET 0~ }"));
    let a = pillar::analyze_source(source);
    serde_json::to_string(a.program.as_ref().unwrap()).unwrap();
}
