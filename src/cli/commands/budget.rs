use crate::api_client::ApiClient;
use crate::error::AppError;

pub async fn show(client: &ApiClient, agent_id: String) -> Result<(), AppError> {
    let json = client.get_agent(&agent_id).await?;
    println!(
        "{}",
        serde_json::json!({
            "agent_id": agent_id,
            "budget_limit_usd": json["budget_limit_usd"],
            "budget_used_usd": json["budget_used_usd"]
        })
    );
    Ok(())
}

pub async fn set(client: &ApiClient, agent_id: String, limit: String) -> Result<(), AppError> {
    let limit: rust_decimal::Decimal = limit
        .parse()
        .map_err(|e| AppError::BadRequest(format!("invalid budget value: {e}")))?;
    let json = client
        .patch_agent(&agent_id, serde_json::json!({"budget_limit_usd": limit}))
        .await?;
    println!("{json}");
    Ok(())
}

pub async fn reset(client: &ApiClient, agent_id: String) -> Result<(), AppError> {
    let json = client
        .patch_agent(&agent_id, serde_json::json!({"budget_limit_usd": 0}))
        .await?;
    println!("{json}");
    Ok(())
}
