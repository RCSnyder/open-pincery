use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::observability::metrics as m;

/// Per-million-token prices (USD). `Default` is zero-cost so tests that
/// don't care about pricing keep working unchanged; real deployments wire
/// real prices via `LlmClient::with_pricing`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Pricing {
    pub input_per_mtok: Decimal,
    pub output_per_mtok: Decimal,
}

impl Pricing {
    pub fn new(input_per_mtok: Decimal, output_per_mtok: Decimal) -> Self {
        Self {
            input_per_mtok,
            output_per_mtok,
        }
    }

    /// Compute cost in USD for the given token usage. Integer-token math keeps
    /// full `Decimal` precision; we divide by 1_000_000 at the end.
    pub fn cost_for(&self, usage: &Usage) -> Decimal {
        let mtok = Decimal::from(1_000_000);
        let input = Decimal::from(usage.prompt_tokens.max(0)) * self.input_per_mtok / mtok;
        let output = Decimal::from(usage.completion_tokens.max(0)) * self.output_per_mtok / mtok;
        input + output
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallRequest>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRequest {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: FunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionDef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDef {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDefinition>>,
}

#[derive(Debug, Deserialize)]
pub struct ChatResponse {
    pub id: String,
    pub choices: Vec<Choice>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
pub struct Choice {
    pub message: ResponseMessage,
    pub finish_reason: String,
}

#[derive(Debug, Deserialize)]
pub struct ResponseMessage {
    pub role: String,
    pub content: Option<String>,
    pub tool_calls: Option<Vec<ToolCallRequest>>,
}

#[derive(Debug, Deserialize)]
pub struct Usage {
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,
}

pub struct LlmClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
    pub model: String,
    pub maintenance_model: String,
    pub primary_pricing: Pricing,
    pub maintenance_pricing: Pricing,
}

impl LlmClient {
    pub fn new(
        base_url: String,
        api_key: String,
        model: String,
        maintenance_model: String,
    ) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url,
            api_key,
            model,
            maintenance_model,
            primary_pricing: Pricing::default(),
            maintenance_pricing: Pricing::default(),
        }
    }

    /// Attach token-price tables for cost accounting. Zero pricing means
    /// `cost_usd` will be recorded as 0 but the insert+increment transaction
    /// still runs, preserving AC-23 atomicity semantics.
    pub fn with_pricing(mut self, primary: Pricing, maintenance: Pricing) -> Self {
        self.primary_pricing = primary;
        self.maintenance_pricing = maintenance;
        self
    }

    /// Compute the USD cost of a single `Usage` report for either the
    /// primary wake-loop model or the maintenance model.
    pub fn estimate_cost(&self, usage: &Usage, is_maintenance: bool) -> Decimal {
        let p = if is_maintenance {
            &self.maintenance_pricing
        } else {
            &self.primary_pricing
        };
        p.cost_for(usage)
    }

    pub async fn chat(
        &self,
        messages: Vec<ChatMessage>,
        tools: Option<Vec<ToolDefinition>>,
        model_override: Option<&str>,
    ) -> Result<ChatResponse, crate::error::AppError> {
        let model = model_override.unwrap_or(&self.model).to_string();

        let request = ChatRequest {
            model,
            messages,
            tools,
        };

        let mut last_err = None;
        for attempt in 0..3 {
            if attempt > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(1000 * (1 << attempt))).await;
            }

            let result = self
                .http
                .post(format!("{}/chat/completions", self.base_url))
                .bearer_auth(&self.api_key)
                .json(&request)
                .send()
                .await;

            match result {
                Ok(resp) => {
                    if resp.status().is_success() {
                        let body = resp.json::<ChatResponse>().await.map_err(|e| {
                            crate::error::AppError::Internal(format!(
                                "LLM response parse error: {e}"
                            ))
                        })?;
                        metrics::counter!(m::LLM_CALL).increment(1);
                        if let Some(u) = body.usage.as_ref() {
                            metrics::counter!(m::LLM_PROMPT_TOKENS)
                                .increment(u.prompt_tokens.max(0) as u64);
                            metrics::counter!(m::LLM_COMPLETION_TOKENS)
                                .increment(u.completion_tokens.max(0) as u64);
                        }
                        return Ok(body);
                    }
                    let status = resp.status();
                    let text = resp.text().await.unwrap_or_default();
                    last_err = Some(format!("LLM API error {status}: {text}"));
                }
                Err(e) => {
                    last_err = Some(format!("LLM request error: {e}"));
                }
            }
        }

        Err(crate::error::AppError::Internal(
            last_err.unwrap_or_else(|| "LLM call failed".into()),
        ))
    }
}
