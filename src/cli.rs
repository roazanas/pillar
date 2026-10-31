use clap::{Parser, Subcommand, ValueEnum};
use std::{ffi::OsString, path::PathBuf};
#[derive(Parser, Debug)]
#[command(
    name = "plrc",
    version,
    long_version = concat!(env!("CARGO_PKG_VERSION"), "\ntarget: ", env!("PILLAR_TARGET")),
    about = "Pillar — a typed, vertical programming language"
)]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<Commands>,
    /// Legacy spelling of `build FILE`.
    pub file: Option<PathBuf>,
    /// Legacy horizontal-to-vertical conversion.
    #[arg(short = 't', long)]
    pub transpose: bool,
    #[arg(short, long, global = true)]
    pub output: Option<PathBuf>,
    #[arg(short, long, global = true)]
    pub verbose: bool,
    #[arg(long, global = true)]
    pub release: bool,
    #[arg(long, global = true)]
    pub linker: Option<OsString>,
    #[arg(long, value_enum, default_value = "auto", global = true)]
    pub color: Color,
    #[arg(long, value_enum, default_value = "human", global = true)]
    pub diagnostic_format: DiagnosticFormat,
}
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Type-check without a native linker.
    Check { file: PathBuf },
    /// Build a native executable or dump an intermediate representation.
    Build {
        file: PathBuf,
        #[arg(long, value_enum, default_value = "exe")]
        emit: Emit,
    },
    /// Build and run; forward arguments and the program's exit status.
    Run {
        file: PathBuf,
        #[arg(last = true, allow_hyphen_values = true)]
        args: Vec<OsString>,
    },
    /// Format to stdout unless --write or --check is supplied.
    Fmt {
        file: PathBuf,
        #[arg(long, conflicts_with = "write")]
        check: bool,
        #[arg(long)]
        write: bool,
    },
    /// Convert a source representation; print to stdout.
    Transpose {
        file: PathBuf,
        #[arg(long = "from", value_enum)]
        from: Representation,
    },
    /// Generate shell completion scripts.
    Completions { shell: clap_complete::Shell },
    /// Run the Language Server Protocol over stdio.
    Lsp,
}
#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq)]
pub enum Color {
    Auto,
    Always,
    Never,
}
#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq)]
pub enum DiagnosticFormat {
    Human,
    Json,
}
#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq)]
pub enum Representation {
    Horizontal,
    Vertical,
}
#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq)]
pub enum Emit {
    Tokens,
    Ast,
    Hir,
    Ir,
    Obj,
    Exe,
}
