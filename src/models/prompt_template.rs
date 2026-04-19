use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct PromptTemplate {
    pub id: Uuid,
    pub name: String,
    pub version: i32,
    pub template: String,
    pub is_active: bool,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub change_reason: Option<String>,
}

pub async fn find_active(pool: &PgPool, name: &str) -> Result<Option<PromptTemplate>, AppError> {
    let tmpl = sqlx::query_as::<_, PromptTemplate>(
        "SELECT * FROM prompt_templates WHERE name = $1 AND is_active = TRUE"
    )
    .bind(name)
    .fetch_optional(pool)
    .await?;
    Ok(tmpl)
}
