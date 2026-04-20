use std::process::ExitCode;

use crate::api_client::ApiClient;
use crate::error::AppError;

pub async fn run(client: &ApiClient) -> Result<ExitCode, AppError> {
    let status = client.ready_status().await?;
    if status.is_success() {
        println!("ready");
        Ok(ExitCode::SUCCESS)
    } else {
        println!("not ready: {}", status.as_u16());
        Ok(ExitCode::from(1))
    }
}
