use std::process::ExitCode;

use clap::{Parser, Subcommand};

use crate::api_client::ApiClient;
use crate::error::AppError;

pub mod commands;
pub mod config;
pub mod migrate;
pub mod nouns;
pub mod output;
pub mod resolve;

#[derive(Parser, Debug)]
#[command(name = "pcy")]
pub struct Cli {
    #[arg(long)]
    url: Option<String>,
    #[arg(long)]
    token: Option<String>,
    /// AC-47 (v8): global output format. Accepts
    /// `table|json|yaml|name|jsonpath=<expr>`. Default is `table` on
    /// a TTY, `json` when stdout is piped. Every data-printing leaf
    /// must honour this flag (enforced by the AC-52b naming lint).
    #[arg(long, global = true, value_parser = parse_output_format)]
    output: Option<output::OutputFormat>,
    /// AC-47 (v8): suppress ANSI colour in `table` output. Equivalent
    /// to setting `NO_COLOR=1` for the lifetime of the invocation.
    #[arg(long = "no-color", global = true)]
    no_color: bool,
    #[command(subcommand)]
    command: Commands,
}

/// Thin adapter between clap's string-in / `Result<T, String>`-out
/// value-parser contract and [`output::OutputFormat`]'s [`FromStr`].
fn parse_output_format(s: &str) -> Result<output::OutputFormat, String> {
    s.parse::<output::OutputFormat>().map_err(|e| e.to_string())
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Authenticate against a Pincery server. Idempotent: on a fresh
    /// server it initialises the admin session; on an already-
    /// bootstrapped server it logs in using the same token (AC-45).
    Login {
        /// Paste a session token directly
        #[arg(long, conflicts_with = "bootstrap_token")]
        token: Option<String>,
        /// Use bootstrap token to obtain a new session
        #[arg(long)]
        bootstrap_token: Option<String>,
    },
    /// Manage agents (create, list, show, disable, rotate secrets).
    Agent {
        #[command(subcommand)]
        command: AgentCommands,
    },
    /// Send a single text message to a named agent and exit.
    Message { agent: String, text: String },
    /// Stream events for a named agent. Use `--tail` to follow live
    /// and `--since <timestamp>` to replay from a checkpoint.
    Events {
        agent: String,
        #[arg(long)]
        tail: bool,
        #[arg(long)]
        since: Option<String>,
    },
    /// Inspect or set per-agent budget limits.
    Budget {
        #[command(subcommand)]
        command: BudgetCommands,
    },
    /// Run an end-to-end smoke test: bootstrap-or-login, create an agent, send
    /// a message, wait for a reply, and print it.
    Demo {
        #[arg(long)]
        bootstrap_token: Option<String>,
    },
    /// Print a one-line health summary of the configured server.
    Status,
    /// AC-40 (v7): manage workspace credentials. Values are prompted
    /// interactively (hidden) or piped via stdin — there is no
    /// `--value` flag so secrets never land in shell history.
    Credential {
        #[command(subcommand)]
        command: CredentialCommands,
    },
    /// AC-48 (v8): manage named connection contexts on disk.
    Context {
        #[command(subcommand)]
        command: nouns::context::ContextCommands,
    },
    /// AC-48 (v8): print current identity — context, user_id,
    /// workspace_id, url — as one JSON line. Exits 0 iff
    /// authenticated; any HTTP or auth failure surfaces non-zero.
    Whoami,
    /// AC-51 (v8): emit a completion script for the named shell on
    /// stdout. Pipe it into your shell's completion directory.
    Completion {
        /// Target shell.
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

#[derive(Subcommand, Debug)]
enum CredentialCommands {
    /// Create or rotate a credential. Prompts for the value with echo
    /// disabled unless `--stdin` is passed.
    Add {
        name: String,
        /// Read the value from stdin instead of prompting interactively.
        #[arg(long)]
        stdin: bool,
    },
    /// List active credential names (no ciphertext).
    List,
    /// Revoke an active credential by name. Requires `--yes` to confirm.
    Revoke {
        name: String,
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Subcommand, Debug)]
enum AgentCommands {
    /// Create a new agent with the given name.
    Create { name: String },
    /// List all agents in the current workspace.
    List,
    /// Show detailed metadata for one agent.
    Show { agent: String },
    /// Disable an agent so it stops receiving work.
    Disable { agent: String },
    /// Rotate the per-agent authentication secret.
    RotateSecret { agent: String },
}

#[derive(Subcommand, Debug)]
enum BudgetCommands {
    /// Show the current budget and spend for one agent.
    Show { agent: String },
    /// Set the budget limit for one agent (e.g. `1.50usd`).
    Set { agent: String, limit: String },
    /// Reset the accumulated spend for one agent to zero.
    Reset { agent: String },
}

fn resolve_url(cli: &Cli, cfg: &config::CliConfig) -> String {
    cli.url
        .clone()
        .or_else(|| std::env::var("OPEN_PINCERY_URL").ok())
        .or_else(|| cfg.url.clone())
        .unwrap_or_else(|| "http://localhost:8080".to_string())
}

fn resolve_token(cli: &Cli, cfg: &config::CliConfig) -> Option<String> {
    cli.token
        .clone()
        .or_else(|| std::env::var("OPEN_PINCERY_TOKEN").ok())
        .or_else(|| cfg.token.clone())
}

pub async fn run() -> ExitCode {
    match run_inner().await {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(1)
        }
    }
}

async fn run_inner() -> Result<ExitCode, AppError> {
    let cli = Cli::parse();
    let cfg = config::load()?;
    let url = resolve_url(&cli, &cfg);
    let token = resolve_token(&cli, &cfg);

    // AC-47: `--no-color` is an alias for `NO_COLOR=1` so every
    // `output::render_table` call (including ones spawned by nested
    // libraries) sees the same contract. Kept as a process-level env
    // mutation rather than an explicit parameter so we don't need to
    // thread a `no_color` bool into every renderer.
    if cli.no_color {
        // Safety: single-threaded code path at CLI entry, before any
        // task spawns. `std::env::set_var` is `unsafe` in Rust 2024
        // but we're on 2021.
        std::env::set_var("NO_COLOR", "1");
    }

    match cli.command {
        Commands::Login {
            token,
            bootstrap_token,
        } => {
            if let Some(bt) =
                bootstrap_token.or_else(|| std::env::var("OPEN_PINCERY_BOOTSTRAP_TOKEN").ok())
            {
                let client = ApiClient::new(url, None);
                commands::login::run_with_bootstrap(&client, bt).await?;
            } else if let Some(t) = token {
                commands::login::run(url, t)?;
            } else {
                return Err(AppError::BadRequest(
                    "pass --token <session_token> or --bootstrap-token <bootstrap_token>".into(),
                ));
            }
            Ok(ExitCode::SUCCESS)
        }
        Commands::Agent { command } => {
            let token = token.clone().ok_or_else(|| {
                AppError::Unauthorized("missing token; run pcy login first".into())
            })?;
            let client = ApiClient::new(url, Some(token));
            match command {
                AgentCommands::Create { name } => commands::agent::create(&client, name).await?,
                AgentCommands::List => commands::agent::list(&client).await?,
                AgentCommands::Show { agent } => commands::agent::show(&client, agent).await?,
                AgentCommands::Disable { agent } => {
                    commands::agent::disable(&client, agent).await?
                }
                AgentCommands::RotateSecret { agent } => {
                    commands::agent::rotate_secret(&client, agent).await?
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        Commands::Message { agent, text } => {
            let token = token.clone().ok_or_else(|| {
                AppError::Unauthorized("missing token; run pcy login first".into())
            })?;
            let client = ApiClient::new(url, Some(token));
            commands::message::run(&client, agent, text).await?;
            Ok(ExitCode::SUCCESS)
        }
        Commands::Events { agent, tail, since } => {
            let token = token.clone().ok_or_else(|| {
                AppError::Unauthorized("missing token; run pcy login first".into())
            })?;
            let client = ApiClient::new(url, Some(token));
            commands::events::run(&client, agent, since, tail).await?;
            Ok(ExitCode::SUCCESS)
        }
        Commands::Budget { command } => {
            let token = token.clone().ok_or_else(|| {
                AppError::Unauthorized("missing token; run pcy login first".into())
            })?;
            let client = ApiClient::new(url, Some(token));
            match command {
                BudgetCommands::Show { agent } => commands::budget::show(&client, agent).await?,
                BudgetCommands::Set { agent, limit } => {
                    commands::budget::set(&client, agent, limit).await?
                }
                BudgetCommands::Reset { agent } => commands::budget::reset(&client, agent).await?,
            }
            Ok(ExitCode::SUCCESS)
        }
        Commands::Status => {
            let client = ApiClient::new(url, token);
            commands::status::run(&client).await
        }
        Commands::Demo { bootstrap_token } => {
            let bootstrap_token = bootstrap_token
                .or_else(|| std::env::var("OPEN_PINCERY_BOOTSTRAP_TOKEN").ok())
                .ok_or_else(|| {
                    AppError::BadRequest(
                        "missing bootstrap token: pass --bootstrap-token or set OPEN_PINCERY_BOOTSTRAP_TOKEN"
                            .into(),
                    )
                })?;
            commands::demo::run(url, bootstrap_token).await?;
            Ok(ExitCode::SUCCESS)
        }
        Commands::Credential { command } => {
            let token = token.clone().ok_or_else(|| {
                AppError::Unauthorized("missing token; run pcy login first".into())
            })?;
            let client = ApiClient::new(url, Some(token));
            match command {
                CredentialCommands::Add { name, stdin } => {
                    commands::credential::add(&client, name, stdin).await?
                }
                CredentialCommands::List => {
                    // AC-47: format defaults to TTY-aware (`table` on a
                    // terminal, `json` when piped). Agentic callers
                    // always get JSON by default; humans on a shell
                    // see a readable table.
                    let fmt = output::default_for_tty(cli.output.clone());
                    commands::credential::list(&client, &fmt).await?
                }
                CredentialCommands::Revoke { name, yes } => {
                    commands::credential::revoke(&client, name, yes).await?
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        Commands::Context { command } => {
            // AC-48 slice 2d-i: pure on-disk verbs, no HTTP.
            // AC-47 slice 2e-a: `--output` flows from the root `Cli`;
            // unset falls back to the TTY-aware default (`table` on a
            // terminal, `json` when piped).
            let path = config::config_path()?;
            let fmt = output::default_for_tty(cli.output.clone());
            let stdout = nouns::context::run(command, &path, &fmt)?;
            if !stdout.is_empty() {
                println!("{stdout}");
            }
            Ok(ExitCode::SUCCESS)
        }
        Commands::Whoami => {
            let token = token.clone().ok_or_else(|| {
                AppError::Unauthorized("missing token; run pcy login first".into())
            })?;
            let client = ApiClient::new(url, Some(token));
            commands::whoami::run(&client, &cfg).await?;
            Ok(ExitCode::SUCCESS)
        }
        Commands::Completion { shell } => {
            commands::completion::run(shell)?;
            Ok(ExitCode::SUCCESS)
        }
    }
}
