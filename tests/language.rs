use pillar::{
    aot_backend::{self, BuildOptions},
    source::SourceFile,
    transposer,
};
use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
fn vertical(text: &str) -> String {
    transposer::transpose(text.lines().collect(), true).join("\n") + "\n"
}
fn analysis(text: &str) -> pillar::Analysis {
    pillar::analyze_source(SourceFile::vertical(0, "test.plr", vertical(text)))
}
fn valid(text: &str) {
    let a = analysis(text);
    assert!(a.success(), "{:?}", a.diagnostics);
}
fn invalid(text: &str, code: &str) {
    let a = analysis(text);
    assert!(!a.success(), "unexpectedly accepted {text}");
    assert_eq!(a.diagnostics[0].code, code, "{:?}", a.diagnostics);
}
fn execute(text: &str, input: &str, release: bool, args: &[&str]) -> (i32, String, String) {
    let dir = tempfile::Builder::new()
        .prefix("pillar test ")
        .tempdir()
        .unwrap();
    let path = dir.path().join("program.plr");
    let source = vertical(text);
    std::fs::write(&path, &source).unwrap();
    let a = pillar::analyze_path(&path);
    assert!(a.success(), "{:?}", a.diagnostics);
    let exe = dir.path().join(if cfg!(windows) {
        "program.exe"
    } else {
        "program"
    });
    aot_backend::build(
        a.program.as_ref().unwrap(),
        &exe,
        &BuildOptions {
            release,
            linker: None,
        },
    )
    .unwrap();
    let mut child = Command::new(&exe)
        .args(args)
        .current_dir(dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() > deadline {
            child.kill().unwrap();
            panic!("program timed out");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let output = child.wait_with_output().unwrap();
    (
        output.status.code().unwrap_or(1),
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    )
}
fn output(text: &str, expected: &str) {
    let (status, out, err) = execute(text, "", false, &[]);
    assert_eq!(status, 0, "{err}");
    assert_eq!(out, expected);
    assert!(err.is_empty(), "{err}");
}
#[test]
fn shipped_examples() {
    let cases = [
        ("fact", "5\n", "120\n".to_owned()),
        ("fib", "10\n", "55\n".to_owned()),
        (
            "mult",
            "",
            (1..=10)
                .flat_map(|i| (1..=10).map(move |j| format!("{}\n", i * j)))
                .collect(),
        ),
        (
            "complex",
            "6\n2\n2.0\n0.0\n",
            "6\n4\n3\n1\n1\n1\n12.5664\n32\n".to_owned(),
        ),
        ("test", "", "5\n".to_owned()),
    ];
    for (name, input, expected) in cases {
        let text = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("examples/{name}.rplr")),
        )
        .unwrap();
        let (status, out, err) = execute(&text, input, false, &[]);
        assert_eq!(status, 0, "{name}: {err}");
        assert_eq!(out, expected, "{name}");
    }
}
#[test]
fn operators_and_typed_calls() {
    output(
        "FN f(x: float, b: bool): float { IF b { RET x * 2.0~ } EL { RET -x~ } } FN main(): int { OUTI 10 - 3 - 2~ OUTI 2 + 3 * 4~ OUTF f(1.5, true)~ IF !(1 == 2) && true || false { OUTI 7~ } RET 0~ }",
        "5\n14\n3\n7\n",
    );
}
#[test]
fn forward_calls_and_mutual_recursion() {
    output(
        "FN main(): int { println(even(8))~ RET 0~ } FN even(n: int): bool { IF n == 0 { RET true~ } EL { RET odd(n - 1)~ } } FN odd(n: int): bool { IF n == 0 { RET false~ } EL { RET even(n - 1)~ } }",
        "true\n",
    );
}
#[test]
fn scopes_and_loops() {
    output(
        "FN early(): int { WH true { RET 7~ } } FN main(): int { LT x: int = 10~ IF true { LT x: int = 20~ OUTI x~ } OUTI x~ OUTI early()~ LT i: int = 0~ WH i < 10 { i = i + 1~ IF i == 2 { continue~ } IF i == 4 { break~ } OUTI i~ } RET 0~ }",
        "20\n10\n7\n1\n3\n",
    );
}
#[test]
fn short_circuit() {
    output(
        "FN explode(): bool { assert(false)~ RET true~ } FN main(): int { println(false && explode())~ println(true || explode())~ RET 0~ }",
        "false\ntrue\n",
    );
}
#[test]
fn strings_and_memory_on_control_flow() {
    output(
        "FN copy(s: str): str { RET s + \"!\"~ } FN main(): int { LT s: str = \"Привет\"~ println(copy(s))~ OUTI len(s)~ OUTI byte_len(s)~ LT i: int = 0~ WH i < 10 { LT t: str = copy(s)~ i = i + 1~ IF i < 9 { continue~ } break~ } println(str(42))~ println(str(1.5))~ RET 0~ }",
        "Привет!\n6\n12\n42\n1.5\n",
    );
}
#[test]
fn arrays_and_structs_have_value_semantics() {
    output(
        "ST Point { x: int, names: [str] } FN main(): int { LT a: [int] = [1, 2]~ LT b: [int] = a~ b[0] = 9~ OUTI a[0]~ OUTI b[0]~ a = push(a, 3)~ OUTI len(a)~ LT p: Point = Point { x: 1, names: [\"old\"] }~ LT q: Point = p~ q.names[0] = \"new\"~ q.x = 7~ println(p.names[0])~ println(q.names[0])~ OUTI p.x~ OUTI q.x~ RET 0~ }",
        "1\n9\n3\nold\nnew\n1\n7\n",
    );
}
#[test]
fn arrays_of_float_and_bool() {
    output(
        "FN floats(a: [float]): [float] { RET a~ } FN main(): int { LT a: [float] = floats([1.5])~ OUTF a[0]~ LT b: [bool] = [true]~ println(b[0])~ LT empty: [str] = []~ empty = push(empty, \"ok\")~ println(empty[0])~ RET 0~ }",
        "1.5\ntrue\nok\n",
    );
}
#[test]
fn invalid_semantics() {
    for (text, code) in [
        ("FN main(): int { LT x: int = 1.5~ RET 0~ }", "E0207"),
        (
            "FN main(): int { IF false { LT x: int = 42~ } OUTI x~ RET 0~ }",
            "E0211",
        ),
        (
            "FN main(): int { LT x: int = 1~ LT x: int = 2~ RET 0~ }",
            "E0201",
        ),
        ("FN main(): int { IF 1 { RET 0~ } RET 0~ }", "E0207"),
        ("FN main(): int { OUTI !2~ RET 0~ }", "E0212"),
        ("FN main(): int { missing()~ RET 0~ }", "E0213"),
        (
            "FN f(x: int): int { RET x~ } FN main(): int { f()~ RET 0~ }",
            "E0215",
        ),
        ("FN main(): int { LT x: int = 1~ }", "E0205"),
        ("FN main(): int { RET 0~ OUTI 1~ }", "E0208"),
        ("FN main(): int { break~ RET 0~ }", "E0210"),
        ("FN main(): int { IF false {} {} RET 0~ }", "E0100"),
        ("FN main(): int { IF false {} EL RET 0~ }", "E0100"),
        ("FN main(): int { println(1 == 2 < 3)~ RET 0~ }", "E0101"),
        ("FN main(): int { RET @~ }", "E0002"),
        ("FN main(): int { RET 0 }", "E0100"),
        ("FN main(): int { LT a: [int] = [true]~ RET 0~ }", "E0207"),
    ] {
        invalid(text, code);
    }
}
#[test]
fn checked_integer_boundaries_in_both_modes() {
    for release in [false, true] {
        for expr in [
            "9223372036854775807 + 1",
            "-9223372036854775808 - 1",
            "-9223372036854775808 * -1",
            "--9223372036854775808",
            "-9223372036854775808 / -1",
            "-9223372036854775808 % -1",
            "1 / 0",
            "1 % 0",
        ] {
            let (status, _, err) = execute(
                &format!("FN main(): int {{ OUTI {expr}~ RET 0~ }}"),
                "",
                release,
                &[],
            );
            assert_ne!(status, 0, "{expr}");
            assert!(err.contains("error[R000"), "{expr}: {err}");
            assert!(err.contains("program.plr:"), "{err}");
        }
    }
    output(
        "FN main(): int { OUTI -9223372036854775808~ OUTI 9223372036854775807~ RET 0~ }",
        "-9223372036854775808\n9223372036854775807\n",
    );
}
#[test]
fn floating_division_by_zero_in_both_modes() {
    for release in [false, true] {
        for expr in ["1.0 / 0.0", "1.0 / -0.0", "0.0 / 0.0"] {
            let (status, _, err) = execute(
                &format!("FN main(): int {{ OUTF {expr}~ RET 0~ }}"),
                "",
                release,
                &[],
            );
            assert_ne!(status, 0, "{expr}");
            assert!(err.contains("error[R0002]"), "{expr}: {err}");
            assert!(err.contains("program.plr:"), "{err}");
        }
    }
    output("FN main(): int { OUTF 7.5 / 2.5~ RET 0~ }", "3\n");
}
#[test]
fn runtime_failures() {
    for body in [
        "LT a: [int] = [1]~ OUTI a[-1]~",
        "OUTI int(1.0e308 * 1.0e308)~",
        "OUTI int(\"no\")~",
        "OUTI int(\"   \")~",
        "OUTF float(\"   \")~",
        "assert(false)~",
    ] {
        let (status, _, err) = execute(
            &format!("FN main(): int {{ {body} RET 0~ }}"),
            "",
            false,
            &[],
        );
        assert_ne!(status, 0);
        assert!(err.contains("error[R"), "{err}");
    }
}
#[test]
fn input_failures_are_not_zero() {
    for (input, code) in [("", "R0010"), ("bad\n", "R0003")] {
        let (status, _, err) = execute(
            "FN main(): int { OUTI read_int()~ RET 0~ }",
            input,
            false,
            &[],
        );
        assert_ne!(status, 0);
        assert!(err.contains(code), "{err}");
    }
}
#[test]
fn program_arguments() {
    let (status, out, err) = execute(
        "FN main(): int { LT a: [str] = args()~ OUTI len(a)~ println(a[0])~ RET 7~ }",
        "",
        false,
        &["hello world"],
    );
    assert_eq!(status, 7, "{err}");
    assert_eq!(out, "1\nhello world\n");
}
#[test]
fn conversion_and_math() {
    output(
        "FN main(): int { OUTI int(-1.9)~ OUTF float(\"1.5\")~ OUTF sqrt(9.0)~ OUTF pow(2.0, 3.0)~ OUTI abs(-7)~ println(\"a\" == \"a\")~ RET 0~ }",
        "-1\n1.5\n3\n8\n7\ntrue\n",
    );
}
#[test]
fn unit_return() {
    output(
        "FN f() { RET~ } FN g() { RET f()~ } FN main(): int { g()~ RET 0~ }",
        "",
    );
}
#[test]
fn imports_and_privacy() {
    let dir = tempfile::tempdir().unwrap();
    let lib = dir.path().join("math.plr");
    std::fs::write(
        &lib,
        vertical("PUB FN twice(x: int): int { RET x * 2~ } FN hidden(): int { RET 0~ }"),
    )
    .unwrap();
    let root = dir.path().join("main.plr");
    std::fs::write(&root,vertical("IM \"math.plr\" AS math~ IM \"std/io\" AS io~ FN main(): int { io::println(math::twice(21))~ RET 0~ }")).unwrap();
    let a = pillar::analyze_path(&root);
    assert!(a.success(), "{:?}", a.diagnostics);
    let exe = dir.path().join("out.exe");
    aot_backend::build(a.program.as_ref().unwrap(), &exe, &BuildOptions::default()).unwrap();
    assert_eq!(Command::new(&exe).output().unwrap().stdout, b"42\n");
    std::fs::write(
        &root,
        vertical("IM \"math.plr\" AS m~ FN main(): int { RET m::hidden()~ }"),
    )
    .unwrap();
    assert_eq!(pillar::analyze_path(&root).diagnostics[0].code, "E0214");
    std::fs::write(&lib, vertical("IM \"main.plr\" AS root~")).unwrap();
    assert_eq!(pillar::analyze_path(&root).diagnostics[0].code, "E0304");
}
#[test]
fn formatter_and_crlf() {
    let source = SourceFile::vertical(
        0,
        "test.plr",
        vertical("// a comment\nFN main(): int { LT x: int = -(2+3)~ OUTI x~ RET 0~ }"),
    );
    let formatted = pillar::formatter::vertical(&source).unwrap();
    let twice =
        pillar::formatter::vertical(&SourceFile::vertical(0, "test.plr", formatted.clone()))
            .unwrap();
    assert_eq!(formatted, twice);
    valid("FN main(): int { RET 0~ }");
    let crlf = source.text.replace('\n', "\r\n");
    assert!(pillar::analyze_source(SourceFile::vertical(0, "test.plr", crlf)).success());
}
#[test]
fn source_positions_and_tabs() {
    let text = vertical("FN main(): int {\n  LT x: str = \"é\"~\n  OUTI missing~\n  RET 0~\n}");
    let source = SourceFile::vertical(0, "positions.plr", text);
    let a = pillar::analyze_source(source.clone());
    let d = &a.diagnostics[0];
    assert_eq!(d.code, "E0211");
    let ranges = source.segments(d.span);
    assert_eq!(ranges.len(), 7);
    for r in ranges {
        assert_eq!(source.position(r.start).column, 5);
    }
    let json = d.json(&a.sources);
    assert_eq!(json["schema_version"], 1);
    let tab = SourceFile::vertical(0, "tab.plr", "\t".into());
    assert_eq!(pillar::analyze_source(tab).diagnostics[0].code, "E0001");
}

#[test]
fn additional_examples_and_file_io() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for (name, input, expected) in [
        ("hello", "", "Hello, Pillar!\nUnicode: 😀\n"),
        (
            "guess",
            "5\n7\n",
            "Guess a number from 1 to 10:\nTry a larger number.\nCorrect!\n",
        ),
        ("collections", "", "Ada\nGrace\n25\n26\n"),
    ] {
        let source = std::fs::read_to_string(root.join(format!("examples/{name}.rplr"))).unwrap();
        let (status, out, err) = execute(&source, input, false, &[]);
        assert_eq!(status, 0, "{err}");
        assert_eq!(out, expected);
    }
    output(
        "FN main(): int { write_file(\"text.txt\", \"Привет😀\")~ LT s: str = read_file(\"text.txt\")~ OUTI len(s)~ OUTI byte_len(s)~ RET 0~ }",
        "7\n16\n",
    );
}
#[test]
fn composite_assignment_evaluates_indices_once() {
    let (status, out, err) = execute(
        "FN main(): int { LT a: [[int]] = [[1]]~ a[read_int()][read_int()] = read_int()~ OUTI a[0][0]~ RET 0~ }",
        "0\n0\n42\n",
        false,
        &[],
    );
    assert_eq!(status, 0, "{err}");
    assert_eq!(out, "42\n");
}
#[test]
fn empty_strings_and_call_depth_limit() {
    output(
        "FN main(): int { LT s: str = \"\"~ println(s)~ OUTI len(s)~ RET 0~ }",
        "\n0\n",
    );
    let (status, _, err) = execute(
        "FN recursive(): int { RET recursive()~ } FN main(): int { RET recursive()~ }",
        "",
        false,
        &[],
    );
    assert_ne!(status, 0);
    assert!(err.contains("R0012"), "{err}");
}
#[test]
fn exported_struct_privacy_and_recursive_types() {
    invalid("ST P { next: [P] } FN main(): int { RET 0~ }", "E0203");
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for path in root
        .join("examples")
        .read_dir()
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "plr"))
    {
        let a = pillar::analyze_path(&path);
        assert!(a.success(), "{}: {:?}", path.display(), a.diagnostics);
        let horizontal = std::fs::read_to_string(path.with_extension("rplr")).unwrap();
        let expected = SourceFile::vertical(0, &path, vertical(&horizontal));
        let actual = SourceFile::vertical(0, &path, std::fs::read_to_string(&path).unwrap());
        let kinds = |s: &SourceFile| {
            pillar::lexer::tokenize(s)
                .unwrap()
                .into_iter()
                .filter(|t| !t.kind.trivia())
                .map(|t| t.kind)
                .collect::<Vec<_>>()
        };
        assert_eq!(kinds(&actual), kinds(&expected), "{}", path.display());
    }
}

#[test]
fn empty_structs_and_diverging_functions() {
    output(
        "ST Empty {} FN yes(e: Empty): bool { RET true~ } FN main(): int { LT e: Empty = Empty {}~ IF yes(Empty {}) { OUTI 1~ } RET 0~ }",
        "1\n",
    );
    valid("FN forever(): int { WH true {} } FN main(): int { RET 0~ }");
    invalid("FN main(): int { WH true {} RET 0~ }", "E0208");
    invalid(
        "FN main(): int { LT len: int = 0~ len(1)~ RET 0~ }",
        "E0221",
    );
}

#[test]
fn input_newlines_and_main_status_are_portable() {
    for input in ["42\nnext\n", "42\r\nnext\r\n"] {
        let (status, out, err) = execute(
            "FN main(): int { OUTI read_int()~ println(read_line())~ RET 0~ }",
            input,
            false,
            &[],
        );
        assert_eq!(status, 0, "{err}");
        assert_eq!(out, "42\nnext\n");
    }
    let (status, _, err) = execute("FN main(): int { RET -1~ }", "", false, &[]);
    assert_eq!(status, 1);
    assert!(err.contains("R0008"));
    output(
        "FN main(): int { OUTI -09223372036854775808~ RET 0~ }",
        "-9223372036854775808\n",
    );
}
#[test]
fn eof_and_large_diagnostics_are_bounded() {
    let source = SourceFile::vertical(0, "eof.plr", vertical("FN main(): int { RET 0~"));
    let a = pillar::analyze_source(source.clone());
    assert_eq!(a.diagnostics[0].code, "E0100");
    assert_eq!(
        source.location(a.diagnostics[0].span).byte,
        source.text.len()
    );
    let a = analysis(&format!(
        "FN main(): int {{ OUTI {}~ RET 0~ }}",
        "a".repeat(300)
    ));
    let json = a.diagnostics[0].json(&a.sources);
    assert_eq!(json["labels"].as_array().unwrap().len(), 256);
    assert_eq!(json["labels_truncated"], true);
    let source = SourceFile::vertical(0, "wide.plr", "a".repeat(3000) + &"\n".repeat(3000));
    let a = pillar::analyze_source(source);
    assert_eq!(a.diagnostics[0].code, "E0003");
}
#[test]
fn concurrent_builds_do_not_share_temporary_files() {
    let a = analysis("FN main(): int { OUTI 42~ RET 0~ }");
    let program = a.program.as_ref().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let executable = dir.path().join("program.exe");
    std::thread::scope(|scope| {
        let first =
            scope.spawn(|| aot_backend::build(program, &executable, &BuildOptions::default()));
        let second =
            scope.spawn(|| aot_backend::build(program, &executable, &BuildOptions::default()));
        first.join().unwrap().unwrap();
        second.join().unwrap().unwrap();
    });
    assert_eq!(Command::new(executable).output().unwrap().stdout, b"42\n");
}

#[test]
fn analysis_assigns_its_own_source_ids() {
    let source = SourceFile::vertical(42, "source.plr", vertical("FN main(): int { RET 0~ }"));
    let a = pillar::analyze_source(source);
    assert!(a.success());
    assert_eq!(a.sources[0].id, 0);
    pillar::compiler::emit(a.program.as_ref().unwrap(), false).unwrap();
}
