use poem_openapi::Object;
use serde::Serialize;
use sqlx::types::chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Object, Serialize, Clone, PartialEq)]
pub struct Payment {
    pub id: Uuid,
    pub account_id: Uuid,

    pub address: String,
    pub amount: f64,
    pub received: f64,

    pub confirmations: u64,
    pub initiated: bool,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
