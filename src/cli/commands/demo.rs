use crate::api_client::ApiClient;
use crate::cli::config::{load, save};
use crate::error::AppError;

const POLL_SECS: u64 = 2;
const MAX_POLLS: u64 = 30; // ~60s total

pub async fn run(url: String, bootstrap_token: String) -> Result<(), AppError> {
    let client = ApiClient::new(url.clone(), None);

    // Step 1: bootstrap-or-login.
    println!("[1/5] Authenticating with bootstrap token...");
    let session_token = match client.bootstrap(&bootstrap_token).await {
        Ok(resp) => resp["session_token"]
            .as_str()
            .ok_or_else(|| AppError::Internal("bootstrap response missing session_token".into()))?
            .to_string(),
        Err(AppError::BadRequest(msg)) if msg.contains("409") => {
            println!("      (system already bootstrapped, logging in)");
            let resp = client.login(&bootstrap_token).await?;
            resp["session_token"]
                .as_str()
                .ok_or_else(|| AppError::Internal("login response missing session_token".into()))?
                .to_string()
        }
        Err(e) => return Err(e),
    };

    // Persist session so subsequent pcy commands work without re-auth.
    let mut cfg = load()?;
    cfg.url = Some(url.clone());
    cfg.token = Some(session_token.clone());
    save(&cfg)?;

    let authed = ApiClient::new(url, Some(session_token));

    // Step 2: create a throwaway demo agent.
    let agent_name = format!(
        "demo-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    );
    println!("[2/5] Creating agent: {agent_name}");
    let agent = authed.create_agent(&agent_name).await?;
    let agent_id = agent["id"]
        .as_str()
        .ok_or_else(|| AppError::Internal("create_agent response missing id".into()))?
        .to_string();
    println!("      agent_id={agent_id}");

    // Step 3: send a prompt.
    let prompt = "Hello! Please introduce yourself in one short sentence, \
                  then tell me what date you think today is.";
    println!("[3/5] Sending prompt: {prompt}");
    authed.send_message(&agent_id, prompt).await?;

    // Step 4: poll events for an assistant_message reply.
    println!("[4/5] Waiting for agent reply (up to 60s)...");
    let mut reply: Option<String> = None;
    for attempt in 1..=MAX_POLLS {
        tokio::time::sleep(std::time::Duration::from_secs(POLL_SECS)).await;
        let json = authed.events(&agent_id, 100, None).await?;
        if let Some(events) = json["events"].as_array() {
            for ev in events {
                if ev["event_type"].as_str() == Some("assistant_message") {
                    if let Some(content) = ev["content"].as_str() {
                        if !content.trim().is_empty() {
                            reply = Some(content.to_string());
                            break;
                        }
                    }
                }
            }
        }
        if reply.is_some() {
            break;
        }
        if attempt % 5 == 0 {
            println!("      still waiting... ({}s elapsed)", attempt * POLL_SECS);
        }
    }

    // Step 5: print outcome.
    println!("[5/5] Result:");
    match reply {
        Some(text) => {
            println!("\n--- Agent reply ---\n{}\n-------------------", text.trim());
            println!(
                "\nOK. The stack works end-to-end. Inspect further with:\n  \
                 pcy agent show {agent_id}\n  \
                 pcy events {agent_id}"
            );
            Ok(())
        }
        None => Err(AppError::Internal(format!(
            "no assistant_message event within {}s. Check logs: docker compose logs -f app. \
             Likely causes: LLM_API_KEY invalid, LLM_API_BASE_URL unreachable, or model name wrong.",
            MAX_POLLS * POLL_SECS
        ))),
    }
}
