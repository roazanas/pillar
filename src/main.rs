mod cli;
use clap::{CommandFactory, Parser};
use cli::*;
use pillar::{
    Analysis,
    aot_backend::{self, BuildOptions},
    compiler,
    diagnostics::Diagnostic,
    formatter,
    source::{SourceFile, Span},
    transposer,
};
use std::{
    io::{self, IsTerminal},
    path::{Path, PathBuf},
    process::Command,
};
fn main() {
    let args = Args::parse();
    if args.verbose {
        let _ =
            env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug"))
                .try_init();
    }
    let status = match execute(&args) {
        Ok(status) => status,
        Err((d, sources)) => {
            report(&args, &d, &sources);
            1
        }
    };
    std::process::exit(status);
}
type Failure = (Diagnostic, Vec<SourceFile>);
fn report(args: &Args, d: &Diagnostic, sources: &[SourceFile]) {
    if args.diagnostic_format == DiagnosticFormat::Json {
        eprintln!("{}", d.json(sources));
    } else {
        let color = args.color == Color::Always
            || (args.color == Color::Auto
                && io::stderr().is_terminal()
                && std::env::var_os("NO_COLOR").is_none());
        eprint!("{}", d.human(sources, color));
    }
}
fn error(code: &str, message: impl Into<String>) -> Failure {
    (Diagnostic::error(code, message, Span::default()), vec![])
}
fn read(file: &Path) -> std::result::Result<String, Failure> {
    pillar::read_source(file)
        .map_err(|e| error("E0300", format!("cannot read {}: {e}", file.display())))
}
fn analyze(file: &Path) -> std::result::Result<Analysis, Failure> {
    if file.extension().is_none_or(|e| e != "plr") {
        return Err(error(
            "E0305",
            "compile .plr files; use transpose --from horizontal to import .rplr",
        ));
    }
    let a = pillar::analyze_path(file);
    if let Some(d) = a.diagnostics.first() {
        return Err((d.clone(), a.sources));
    }
    Ok(a)
}
fn check_output(path: &Path, sources: &[SourceFile]) -> std::result::Result<(), Failure> {
    let out = std::fs::canonicalize(path).ok();
    if out.is_some()
        && sources
            .iter()
            .any(|s| std::fs::canonicalize(&s.path).ok() == out)
    {
        return Err(error("E0501", "output path would overwrite a source file"));
    }
    Ok(())
}
fn build(args: &Args, file: &Path, emit: Emit) -> std::result::Result<i32, Failure> {
    let a = analyze(file)?;
    let p = a.program.as_ref().expect("checked program");
    let options = BuildOptions {
        release: args.release,
        linker: args.linker.clone(),
    };
    match emit {
        Emit::Tokens => println!(
            "{}",
            serde_json::to_string_pretty(&a.tokens).map_err(|e| error("E9000", e.to_string()))?
        ),
        Emit::Ast => println!(
            "{}",
            serde_json::to_string_pretty(&a.ast).map_err(|e| error("E9000", e.to_string()))?
        ),
        Emit::Hir => println!(
            "{}",
            serde_json::to_string_pretty(p).map_err(|e| error("E9000", e.to_string()))?
        ),
        Emit::Ir | Emit::Obj => {
            let object = compiler::emit(p, args.release).map_err(|d| (d, a.sources.clone()))?;
            if emit == Emit::Ir {
                print!("{}", object.ir);
            } else {
                let output = args
                    .output
                    .clone()
                    .unwrap_or_else(|| PathBuf::from("out.o"));
                check_output(&output, &a.sources)?;
                aot_backend::atomic_write(&output, &object.bytes, None)
                    .map_err(|e| error("E0500", e.to_string()))?;
            }
        }
        Emit::Exe => {
            let output = args
                .output
                .clone()
                .unwrap_or_else(|| PathBuf::from(if cfg!(windows) { "out.exe" } else { "out" }));
            check_output(&output, &a.sources)?;
            aot_backend::build(p, &output, &options).map_err(|d| (d, a.sources.clone()))?;
        }
    }
    Ok(0)
}
fn transpose(file: &Path, from: Representation) -> std::result::Result<i32, Failure> {
    let text = read(file)?;
    let source = SourceFile::vertical(0, file, text.clone());
    if let Some(message) = source.layout_error {
        return Err(error("E0003", message));
    }
    let rows = transposer::transpose(text.lines().collect(), from == Representation::Horizontal);
    let output = rows
        .iter()
        .map(|row| row.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    if from == Representation::Horizontal {
        let result = SourceFile::vertical(0, file, output.clone());
        if let Some(message) = result.layout_error {
            return Err(error("E0003", message));
        }
    }
    print!("{output}");
    Ok(0)
}
fn execute(args: &Args) -> std::result::Result<i32, Failure> {
    let Some(command) = &args.command else {
        if let Some(file) = &args.file {
            return if args.transpose {
                transpose(file, Representation::Horizontal)
            } else {
                build(args, file, Emit::Exe)
            };
        }
        Args::command()
            .print_help()
            .map_err(|e| error("E0300", e.to_string()))?;
        println!();
        return Ok(0);
    };
    match command {
        Commands::Check { file } => {
            analyze(file)?;
            Ok(0)
        }
        Commands::Build { file, emit } => build(args, file, *emit),
        Commands::Run {
            file,
            args: program_args,
        } => {
            let a = analyze(file)?;
            let dir = tempfile::tempdir().map_err(|e| error("E0500", e.to_string()))?;
            let output = dir.path().join(if cfg!(windows) {
                "program.exe"
            } else {
                "program"
            });
            let options = BuildOptions {
                release: args.release,
                linker: args.linker.clone(),
            };
            aot_backend::build(
                a.program.as_ref().expect("checked program"),
                &output,
                &options,
            )
            .map_err(|d| (d, a.sources))?;
            let status = Command::new(output)
                .args(program_args)
                .status()
                .map_err(|e| error("E0502", format!("cannot run program: {e}")))?;
            Ok(status.code().unwrap_or(1))
        }
        Commands::Fmt { file, check, write } => {
            if file.extension().is_none_or(|e| e != "plr") {
                return Err(error("E0305", "fmt expects a .plr file"));
            }
            let text = read(file)?;
            let source = SourceFile::vertical(0, file, text);
            let formatted = formatter::vertical(&source).map_err(|d| (d, vec![source.clone()]))?;
            if *check {
                return Ok(i32::from(formatted != source.text));
            }
            if *write {
                aot_backend::atomic_write(file, formatted.as_bytes(), None)
                    .map_err(|e| error("E0300", e.to_string()))?;
            } else {
                print!("{formatted}");
            }
            Ok(0)
        }
        Commands::Transpose { file, from } => transpose(file, *from),
        Commands::Completions { shell } => {
            clap_complete::generate(*shell, &mut Args::command(), "plrc", &mut io::stdout());
            Ok(0)
        }
        Commands::Lsp => {
            pillar::lsp::serve(io::stdin().lock(), io::stdout().lock())
                .map_err(|e| error("E0600", e.to_string()))?;
            Ok(0)
        }
    }
}
