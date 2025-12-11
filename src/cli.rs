use std::path::PathBuf;

use clap::Parser as CliParser;

#[derive(Debug, CliParser)]
#[command(version, about = "Compiler for vertical esolang pillar")]
pub struct Args {
    pub file: PathBuf,
    #[arg(short, long, default_value = "out")]
    pub output: PathBuf,
    #[arg(short, long)]
    pub verbose: bool,
    #[arg(short, long)]
    pub transpose: bool,
}
