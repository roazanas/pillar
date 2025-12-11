use std::path::PathBuf;

use clap::Parser as CliParser;
use clap::builder::styling::{AnsiColor, Effects, Styles};

const STYLES: Styles = Styles::styled()
    .header(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .usage(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .literal(AnsiColor::Cyan.on_default().effects(Effects::BOLD))
    .placeholder(AnsiColor::Cyan.on_default());

#[derive(Debug, CliParser)]
#[command(
    version, 
    about = "Compiler for vertical esolang pillar",
    styles = STYLES,
)]
pub struct Args {
    pub file: PathBuf,
    #[arg(short, long, default_value = "out")]
    pub output: PathBuf,
    #[arg(short, long)]
    pub verbose: bool,
    #[arg(short, long)]
    pub transpose: bool,
}
