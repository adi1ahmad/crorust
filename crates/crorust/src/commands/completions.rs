use clap::CommandFactory;
use clap_complete::{generate as clap_gen, Shell};
use std::io::Write;

pub fn generate(shell: Shell) {
    let mut cmd = crate::cli::CliArguments::command();
    let bin_name = cmd.get_name().to_string();
    let mut buf = Vec::new();
    clap_gen(shell, &mut cmd, bin_name, &mut buf);
    let _ = std::io::stdout().write_all(&buf);
}
