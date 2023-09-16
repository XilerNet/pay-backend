use chrono::NaiveDateTime;
use poem_openapi::Object;
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Object, Serialize, Clone, PartialEq)]
pub struct Payment {
    pub id: Uuid,
    pub account_id: Uuid,

    pub address: String,
    pub amount: f64,
    pub received: f64,

    pub confirmations: i32,
    pub initiated: bool,
    pub completed: bool,

    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}
