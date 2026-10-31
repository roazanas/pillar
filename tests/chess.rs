use pillar::{
    aot_backend::{self, BuildOptions},
    formatter, lexer,
    source::SourceFile,
};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

fn directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/chess")
}

#[test]
fn chess_sources_and_readable_views_stay_in_sync() {
    for name in [
        "position", "moves", "search", "fixtures", "selftest", "ui", "main",
    ] {
        let path = directory().join(format!("{name}.plr"));
        let text = std::fs::read_to_string(&path).unwrap();
        let source = SourceFile::vertical(0, &path, text.clone());
        assert_eq!(formatter::vertical(&source).unwrap(), text, "{name}");
        let view = SourceFile::horizontal(
            0,
            path.with_extension("rplr"),
            std::fs::read_to_string(path.with_extension("rplr")).unwrap(),
        );
        let tokens = |source: &SourceFile| {
            lexer::tokenize(source)
                .unwrap()
                .into_iter()
                .filter(|token| !token.kind.trivia())
                .map(|token| token.kind)
                .collect::<Vec<_>>()
        };
        assert_eq!(tokens(&source), tokens(&view), "{name}");
    }
}

fn run(exe: &Path, cwd: &Path, args: &[&str], input: &str) -> (i32, String, String) {
    let mut child = Command::new(exe)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let capture = |pipe: Box<dyn Read + Send>| {
        thread::spawn(move || {
            let mut bytes = Vec::new();
            pipe.take(131_073).read_to_end(&mut bytes).unwrap();
            assert!(bytes.len() <= 131_072, "chess output exceeded 128 KiB");
            String::from_utf8(bytes).unwrap()
        })
    };
    let stdout = capture(Box::new(child.stdout.take().unwrap()));
    let stderr = capture(Box::new(child.stderr.take().unwrap()));
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() > deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("chess timed out: {args:?}");
        }
        thread::sleep(Duration::from_millis(10));
    };
    (
        status.code().unwrap_or(1),
        stdout.join().unwrap(),
        stderr.join().unwrap(),
    )
}

#[test]
fn chess_native_rules_search_and_console_work_together() {
    let analysis = pillar::analyze_path(&directory().join("main.plr"));
    assert!(analysis.success(), "{:?}", analysis.diagnostics);
    let temporary = tempfile::Builder::new()
        .prefix("pillar chess ")
        .tempdir()
        .unwrap();
    let exe = temporary
        .path()
        .join(if cfg!(windows) { "chess.exe" } else { "chess" });
    aot_backend::build(
        analysis.program.as_ref().unwrap(),
        &exe,
        &BuildOptions {
            release: true,
            linker: None,
        },
    )
    .unwrap();
    let success = |args: &[&str], input: &str| {
        let (status, out, err) = run(&exe, temporary.path(), args, input);
        assert_eq!(status, 0, "{args:?}: {out}\n{err}");
        assert!(err.is_empty(), "{args:?}: {err}");
        out
    };

    let out = success(&["selftest", "full"], "");
    assert!(out.ends_with("SELFTEST OK\n"));
    assert_eq!(
        out.lines().filter(|line| line.starts_with("PASS ")).count(),
        43
    );
    assert_eq!(success(&["perft", "0"], ""), "1\n");
    assert_eq!(success(&["perft", "3", "kiwipete"], ""), "97862\n");
    let divided = success(&["divide", "2"], "");
    assert!(divided.contains("e2e4 20\n"));
    assert!(divided.ends_with("total 400\n"));

    let out = success(
        &["play", "1", "both"],
        "e2e5\nnonsense\nmoves\ne2e4\nundo\nd2d4\nsave\nquit\n",
    );
    assert_eq!(out.matches("Illegal move or unknown command").count(), 2);
    assert!(out.contains("Undid 1 ply."));
    assert!(out.contains("Game closed."));
    let transcript = std::fs::read_to_string(temporary.path().join("pillar-chess.txt")).unwrap();
    assert!(transcript.contains("1. d2d4"));
    assert!(!transcript.contains("e2e4"));

    assert!(success(&["play", "1", "white"], "e2e4\nquit\n").contains("Engine plays "));
    assert!(
        success(&["play", "1", "both"], "f2f3\ne7e5\ng2g4\nd8h4\n")
            .contains("CHECKMATE: Black wins.")
    );
    let cycle = "g1f3\ng8f6\nf3g1\nf6g8\n";
    assert!(
        success(&["play", "1", "both"], &(cycle.repeat(2) + "claim\n"))
            .contains("DRAW: claim accepted.")
    );
    assert!(
        success(&["play", "1", "both"], &cycle.repeat(4)).contains("DRAW: fivefold repetition.")
    );
    assert!(success(&["selfplay", "1", "12"], "").ends_with("SELFPLAY OK\n"));
    assert!(success(&["replay", "e2e4", "e7e5", "g1f3"], "").ends_with("REPLAY OK\n"));
    let analysis = success(&["analyze", "2"], "");
    assert!(analysis.contains("depth 2 nodes "));
    assert!(analysis.contains("best "));
    for args in [
        vec!["perft", "-1"],
        vec!["analyze", "999"],
        vec!["perft", "3", "missing"],
        vec!["selfplay", "1", "0"],
        vec!["play", "1", "green"],
        vec!["replay", "e2e5"],
        vec!["selftest", "unknown"],
    ] {
        let (status, _, err) = run(&exe, temporary.path(), &args, "");
        assert_eq!(status, 2, "{args:?}");
        assert!(err.is_empty(), "{err}");
    }
}
