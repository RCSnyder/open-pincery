use crate::api_client::ApiClient;
use crate::error::AppError;

pub async fn run(client: &ApiClient, agent_id: String, text: String) -> Result<(), AppError> {
    let json = client.send_message(&agent_id, &text).await?;
    println!("{json}");
    Ok(())
}
