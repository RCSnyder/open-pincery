//! AC-51 (v8): `pcy completion <shell>` — emit shell completion
//! scripts via `clap_complete`.
//!
//! Usage:
//!   pcy completion bash       > /etc/bash_completion.d/pcy
//!   pcy completion zsh        > "${fpath[1]}/_pcy"
//!   pcy completion fish       > ~/.config/fish/completions/pcy.fish
//!   pcy completion powershell >> $PROFILE
//!
//! The heavy lifting is `clap_complete::generate(Shell::X, &mut cmd,
//! "pcy", &mut stdout)`; this module owns nothing else.

use clap::CommandFactory;
use clap_complete::{generate, Shell};

use crate::cli::Cli;
use crate::error::AppError;

pub fn run(shell: Shell) -> Result<(), AppError> {
    let mut cmd = Cli::command();
    let bin_name = cmd.get_name().to_string();
    generate(shell, &mut cmd, bin_name, &mut std::io::stdout());
    Ok(())
}
