use clap_complete::{generate_to, shells::Shell};
use std::env;

include!("src/cli.rs");

fn main() {
    let outdir = env::var_os("CARGO_MANIFEST_DIR").unwrap();

    let mut cmd = <Args as clap::CommandFactory>::command_for_update();
    cmd.set_bin_name("plrc");

    let shells = [Shell::Bash, Shell::Zsh, Shell::Fish, Shell::PowerShell];
    for shell in shells {
        generate_to(shell, &mut cmd, "plrc", &outdir).expect("Error generating autocomplete");
    }

    println!("cargo:rerun-if-changed=src/cli.rs");
}
