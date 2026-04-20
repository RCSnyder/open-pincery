use crate::api_client::ApiClient;
use crate::error::AppError;

pub async fn create(client: &ApiClient, name: String) -> Result<(), AppError> {
    let json = client.create_agent(&name).await?;
    println!("{json}");
    Ok(())
}

pub async fn list(client: &ApiClient) -> Result<(), AppError> {
    let json = client.list_agents().await?;
    println!("{json}");
    Ok(())
}

pub async fn show(client: &ApiClient, agent_id: String) -> Result<(), AppError> {
    let json = client.get_agent(&agent_id).await?;
    println!("{json}");
    Ok(())
}

pub async fn disable(client: &ApiClient, agent_id: String) -> Result<(), AppError> {
    let json = client
        .patch_agent(&agent_id, serde_json::json!({"is_enabled": false}))
        .await?;
    println!("{json}");
    Ok(())
}

pub async fn rotate_secret(client: &ApiClient, agent_id: String) -> Result<(), AppError> {
    let json = client.rotate_webhook_secret(&agent_id).await?;
    println!("{json}");
    Ok(())
}
