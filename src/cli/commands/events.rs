use crate::api_client::ApiClient;
use crate::error::AppError;

pub async fn run(
    client: &ApiClient,
    agent_id: String,
    since: Option<String>,
    tail: bool,
) -> Result<(), AppError> {
    if tail {
        let mut last_seen = since;
        loop {
            let json = client.events(&agent_id, 100).await?;
            if let Some(events) = json["events"].as_array() {
                let mut started = last_seen.is_none();
                for ev in events.iter().rev() {
                    let id = ev["id"].as_str().unwrap_or_default().to_string();
                    if let Some(s) = last_seen.as_ref() {
                        if !started {
                            if &id == s {
                                started = true;
                            }
                            continue;
                        }
                    }
                    println!("{ev}");
                    last_seen = Some(id);
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    }

    let json = client.events(&agent_id, 200).await?;
    if let Some(marker) = since {
        if let Some(events) = json["events"].as_array() {
            for ev in events.iter().rev() {
                if ev["id"].as_str() == Some(marker.as_str()) {
                    continue;
                }
                println!("{ev}");
            }
            return Ok(());
        }
    }

    println!("{json}");
    Ok(())
}
