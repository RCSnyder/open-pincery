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
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Bootstrap {
        #[arg(long)]
        bootstrap_token: Option<String>,
    },
    Login {
        /// Paste a session token directly
        #[arg(long, conflicts_with = "bootstrap_token")]
        token: Option<String>,
        /// Use bootstrap token to obtain a new session
        #[arg(long)]
        bootstrap_token: Option<String>,
    },
    Agent {
        #[command(subcommand)]
        command: AgentCommands,
    },
    Message {
        agent: String,
        text: String,
    },
    Events {
        agent: String,
        #[arg(long)]
        tail: bool,
        #[arg(long)]
        since: Option<String>,
    },
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
    Create { name: String },
    List,
    Show { agent: String },
    Disable { agent: String },
    RotateSecret { agent: String },
}

#[derive(Subcommand, Debug)]
enum BudgetCommands {
    Show { agent: String },
    Set { agent: String, limit: String },
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

    match cli.command {
        Commands::Bootstrap { bootstrap_token } => {
            let bootstrap_token = bootstrap_token
                .or_else(|| std::env::var("OPEN_PINCERY_BOOTSTRAP_TOKEN").ok())
                .ok_or_else(|| {
                    AppError::BadRequest(
                        "missing bootstrap token: pass --bootstrap-token or OPEN_PINCERY_BOOTSTRAP_TOKEN"
                            .into(),
                    )
                })?;
            let client = ApiClient::new(url, None);
            commands::bootstrap::run(&client, bootstrap_token).await?;
            Ok(ExitCode::SUCCESS)
        }
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
                CredentialCommands::List => commands::credential::list(&client).await?,
                CredentialCommands::Revoke { name, yes } => {
                    commands::credential::revoke(&client, name, yes).await?
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        Commands::Context { command } => {
            // AC-48 slice 2d-i: pure on-disk verbs, no HTTP. Uses the
            // default output format (table when TTY, json when piped)
            // until slice 2e wires `--output` to the root `Cli`.
            let path = config::config_path()?;
            let fmt = output::default_for_tty(None);
            let stdout = nouns::context::run(command, &path, &fmt)?;
            if !stdout.is_empty() {
                println!("{stdout}");
            }
            Ok(ExitCode::SUCCESS)
        }
    }
}
