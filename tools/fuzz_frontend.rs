//! Deterministic grammar-seeded mutation fuzzing, available on stable Rust.
use pillar::{analyze_source, formatter, source::SourceFile, transposer};
use std::collections::BTreeMap;
fn next(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    *state
}
fn main() {
    let count = std::env::args()
        .nth(1)
        .and_then(|n| n.parse::<usize>().ok())
        .unwrap_or(10_000);
    let mut state = std::env::args()
        .nth(2)
        .and_then(|n| n.parse::<u64>().ok())
        .unwrap_or(17);
    let initial = state;
    let seeds = [
        "FN main(): int { RET 0~ }",
        "FN main(): int { LT a: [str] = [\"a\"]~ a[0] = \"b\"~ println(a[0])~ RET 0~ }",
        "// comment\nFN f(x: float): float { RET -x~ }\nFN main(): int { OUTF f(1.5)~ RET 0~ }",
        "ST P { x: int } FN main(): int { LT p: P = P { x: 1 }~ WH p.x < 4 { p.x = p.x + 1~ } RET 0~ }",
        "FN main(): int { IF true { RET 0~ } EL { RET 1~ } }",
    ];
    let alphabet = b"FN LT RET IF EL WH (){}[]:~,=+-*/%\"\\0123456789abc\n\t@";
    let mut codes = BTreeMap::<String, usize>::new();
    let mut checked: usize = 0;
    let mut formatted: usize = 0;
    for _ in 0..count {
        let seed = seeds[next(&mut state) as usize % seeds.len()];
        let mut bytes = transposer::transpose(seed.lines().collect(), true)
            .join("\n")
            .into_bytes();
        let mutations = next(&mut state) as usize % 8;
        for _ in 0..mutations {
            let index = next(&mut state) as usize % (bytes.len() + 1);
            match next(&mut state) % 3 {
                0 if index < bytes.len() => {
                    bytes.remove(index);
                }
                1 => bytes.insert(index, alphabet[next(&mut state) as usize % alphabet.len()]),
                _ if index < bytes.len() => {
                    bytes[index] = alphabet[next(&mut state) as usize % alphabet.len()]
                }
                _ => {}
            }
        }
        let Ok(text) = String::from_utf8(bytes) else {
            continue;
        };
        let sample = text.clone();
        let result = std::panic::catch_unwind(|| {
            let source = SourceFile::vertical(0, "fuzz.plr", text);
            let a = analyze_source(source.clone());
            if let Some(d) = a.diagnostics.first() {
                Some(d.code.clone())
            } else {
                let first = formatter::vertical(&source).expect("valid program must format");
                let second =
                    formatter::vertical(&SourceFile::vertical(0, "fuzz.plr", first.clone()))
                        .expect("formatted program must parse");
                assert_eq!(first, second, "formatter must be idempotent");
                None
            }
        });
        match result {
            Ok(Some(code)) => *codes.entry(code).or_default() += 1,
            Ok(None) => {
                checked += 1;
                formatted += 1;
            }
            Err(_) => {
                let _ = std::fs::create_dir_all("target");
                let _ = std::fs::write("target/fuzz-crash.plr", sample);
                eprintln!(
                    "frontend fuzz failure; seed {initial}, state {state}; see target/fuzz-crash.plr"
                );
                std::process::exit(1);
            }
        }
    }
    println!(
        "{}",
        serde_json::json!({"cases":count,"seed":initial,"valid_programs":checked,"formatted_programs":formatted,"diagnostic_codes":codes})
    );
}
