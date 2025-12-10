use clap_complete::{generate_to, shells::Shell};

include!("src/cli.rs");

fn main() {
    let outdir = "target/completions";
    std::fs::create_dir_all(outdir).expect("Cannot create dir for autocomplete");

    let mut cmd = <Args as clap::CommandFactory>::command_for_update();
    cmd.set_bin_name("plrc");

    let shells = [Shell::Bash, Shell::Zsh, Shell::Fish, Shell::PowerShell];
    for shell in shells {
        generate_to(shell, &mut cmd, "plrc", outdir).expect("Error generating autocomplete");
    }

    println!("cargo:rerun-if-changed=src/cli.rs");
}
