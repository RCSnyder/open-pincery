use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    open_pincery::cli::run().await
}
