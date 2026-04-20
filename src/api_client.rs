use reqwest::StatusCode;
use serde_json::Value;

use crate::error::AppError;

#[derive(Clone)]
pub struct ApiClient {
    http: reqwest::Client,
    pub base_url: String,
    pub token: Option<String>,
}

impl ApiClient {
    pub fn new(base_url: String, token: Option<String>) -> Self {
        Self {
            http: reqwest::Client::builder()
                .no_proxy()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("reqwest client with timeout"),
            base_url: base_url.trim_end_matches('/').to_string(),
            token,
        }
    }

    async fn send_json(
        &self,
        req: reqwest::RequestBuilder,
        body: Option<Value>,
    ) -> Result<Value, AppError> {
        let req = if let Some(token) = self.token.as_ref() {
            req.bearer_auth(token)
        } else {
            req
        };
        let req = if let Some(v) = body {
            req.json(&v)
        } else {
            req
        };

        let resp = req
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("request failed: {e:?}")))?;
        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| AppError::Internal(format!("response read failed: {e}")))?;

        if !status.is_success() {
            return Err(AppError::BadRequest(format!(
                "HTTP {}: {}",
                status.as_u16(),
                text
            )));
        }

        serde_json::from_str(&text)
            .map_err(|e| AppError::Internal(format!("invalid json response: {e}; body={text}")))
    }

    pub async fn bootstrap(&self, bootstrap_token: &str) -> Result<Value, AppError> {
        let req = self
            .http
            .post(format!("{}/api/bootstrap", self.base_url))
            .header("Authorization", format!("Bearer {bootstrap_token}"));
        self.send_json(
            req,
            Some(serde_json::json!({
                "email": "admin@localhost",
                "display_name": "Admin"
            })),
        )
        .await
    }

    pub async fn list_agents(&self) -> Result<Value, AppError> {
        let req = self.http.get(format!("{}/api/agents", self.base_url));
        self.send_json(req, None).await
    }

    pub async fn create_agent(&self, name: &str) -> Result<Value, AppError> {
        let req = self.http.post(format!("{}/api/agents", self.base_url));
        self.send_json(req, Some(serde_json::json!({ "name": name })))
            .await
    }

    pub async fn get_agent(&self, agent_id: &str) -> Result<Value, AppError> {
        let req = self
            .http
            .get(format!("{}/api/agents/{}", self.base_url, agent_id));
        self.send_json(req, None).await
    }

    pub async fn patch_agent(&self, agent_id: &str, body: Value) -> Result<Value, AppError> {
        let req = self
            .http
            .patch(format!("{}/api/agents/{}", self.base_url, agent_id));
        self.send_json(req, Some(body)).await
    }

    pub async fn rotate_webhook_secret(&self, agent_id: &str) -> Result<Value, AppError> {
        let req = self.http.post(format!(
            "{}/api/agents/{}/webhook/rotate",
            self.base_url, agent_id
        ));
        self.send_json(req, None).await
    }

    pub async fn send_message(&self, agent_id: &str, text: &str) -> Result<Value, AppError> {
        let req = self.http.post(format!(
            "{}/api/agents/{}/messages",
            self.base_url, agent_id
        ));
        self.send_json(req, Some(serde_json::json!({ "content": text })))
            .await
    }

    pub async fn events(&self, agent_id: &str, limit: i64) -> Result<Value, AppError> {
        let req = self.http.get(format!(
            "{}/api/agents/{}/events?limit={}",
            self.base_url, agent_id, limit
        ));
        self.send_json(req, None).await
    }

    pub async fn ready_status(&self) -> Result<StatusCode, AppError> {
        let resp = self
            .http
            .get(format!("{}/ready", self.base_url))
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("request failed: {e:?}")))?;
        Ok(resp.status())
    }
}
