use crate::api_client::ApiClient;
use crate::error::AppError;

const EVENT_PAGE_LIMIT: i64 = 1000;

async fn fetch_incremental_events(
    client: &ApiClient,
    agent_id: &str,
    since: String,
) -> Result<Vec<serde_json::Value>, AppError> {
    let mut cursor = since;
    let mut collected = Vec::new();

    loop {
        let json = client
            .events(agent_id, EVENT_PAGE_LIMIT, Some(cursor.as_str()))
            .await?;
        let Some(events) = json["events"].as_array() else {
            break;
        };
        if events.is_empty() {
            break;
        }

        collected.extend(events.iter().cloned());
        let last_id = events
            .last()
            .and_then(|ev| ev["id"].as_str())
            .ok_or_else(|| AppError::Internal("event response missing id".into()))?;
        cursor = last_id.to_string();

        if events.len() < EVENT_PAGE_LIMIT as usize {
            break;
        }
    }

    Ok(collected)
}

pub async fn run(
    client: &ApiClient,
    agent_id: String,
    since: Option<String>,
    tail: bool,
) -> Result<(), AppError> {
    if tail {
        let mut last_seen = since;
        loop {
            if let Some(cursor) = last_seen.clone() {
                let events = fetch_incremental_events(client, &agent_id, cursor).await?;
                for ev in &events {
                    println!("{ev}");
                }
                if let Some(last_id) = events.last().and_then(|ev| ev["id"].as_str()) {
                    last_seen = Some(last_id.to_string());
                }
            } else {
                let json = client.events(&agent_id, EVENT_PAGE_LIMIT, None).await?;
                if let Some(events) = json["events"].as_array() {
                    for ev in events.iter().rev() {
                        println!("{ev}");
                    }
                    if let Some(first_id) = events.first().and_then(|ev| ev["id"].as_str()) {
                        last_seen = Some(first_id.to_string());
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    }

    if let Some(marker) = since {
        let events = fetch_incremental_events(client, &agent_id, marker).await?;
        for ev in &events {
            println!("{ev}");
        }
        return Ok(());
    }

    let json = client.events(&agent_id, 200, None).await?;
    println!("{json}");
    Ok(())
}
