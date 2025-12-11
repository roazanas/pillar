use std::path::PathBuf;

use clap::Parser as CliParser;
use clap::builder::styling::{AnsiColor, Effects, Styles};
use owo_colors::OwoColorize;

const STYLES: Styles = Styles::styled()
    .header(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .usage(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .literal(AnsiColor::Cyan.on_default().effects(Effects::BOLD))
    .placeholder(AnsiColor::Cyan.on_default());

fn get_about() -> String {
    let indent = " ".repeat(12);

    format!(
        "{}\n{}{}\n{}{} {} {} {} {} {}\n{}{} {} {} {} {} {}\n{}{} {} {} {} {} {}\n{}{} {} {} {} {} {}\n{}{}",
        "Compiler for vertical esolang named".green(),
        indent,
        "p i l l a r".black().bold().on_white(),
        indent,
        "i".bright_black(),
        "l".black().bold().on_white(),
        "l a".bright_black(),
        "r".black().bold().on_white(),
        "p".bright_black(),
        "",
        indent,
        "l".bright_black(),
        "l".black().bold().on_white(),
        "a r".bright_black(),
        "p".black().bold().on_white(),
        "i".bright_black(),
        "",
        indent,
        "l".bright_black(),
        "a".black().bold().on_white(),
        "r p".bright_black(),
        "i".black().bold().on_white(),
        "l".bright_black(),
        "",
        indent,
        "a".bright_black(),
        "r".black().bold().on_white(),
        "p i".bright_black(),
        "l".black().bold().on_white(),
        "l".bright_black(),
        "",
        indent,
        "r p i l l a".black().bold().on_white()
    )
}

#[derive(Debug, CliParser)]
#[command(
    version,
    about = get_about(),
    styles = STYLES,
)]
pub struct Args {
    /// Path to source file
    pub file: PathBuf,

    /// Executable output file path
    #[arg(short, long, default_value = "out")]
    pub output: PathBuf,

    /// Enable logs from lexer, parser and cranelift
    #[arg(short, long)]
    pub verbose: bool,

    /// Transpose source file, print to stdout and exit
    #[arg(short, long)]
    pub transpose: bool,
}
