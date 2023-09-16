use uuid::Uuid;

use crate::db::{log::LogTypes, repositories::models::payment::Payment};

pub trait PaymentRepository
where
    Self: Clone,
{
    async fn new() -> Self;

    async fn add_log(
        &self,
        account_id: &Uuid,
        log_type: LogTypes,
        log_data: Option<&str>,
    ) -> Result<(), sqlx::Error>;

    async fn add_payment_confirmation(
        &self,
        payment_id: &Uuid,
        confirmations: u64,
    ) -> Result<(), sqlx::Error>;

    async fn add_payment_received(
        &self,
        payment_id: &Uuid,
        received: f64,
    ) -> Result<(), sqlx::Error>;

    async fn complete_payment(&self, payment_id: &Uuid) -> Result<(), sqlx::Error>;

    async fn create_payment(
        &self,
        account_id: &Uuid,
        address: &str,
        amount: f64,
    ) -> Result<Uuid, sqlx::Error>;

    async fn create_payment_inscription(
        &self,
        payment_id: &Uuid,
        target: &str,
        contents: &str,
    ) -> Result<(), sqlx::Error>;

    async fn initiate_payment(&self, payment_id: &Uuid) -> Result<(), sqlx::Error>;

    async fn get_to_be_initiated_payments(&self) -> Result<Vec<Uuid>, sqlx::Error>;

    async fn get_to_be_completed_payments(
        &self,
        min_confirmations: usize,
    ) -> Result<Vec<Uuid>, sqlx::Error>;

    async fn get_payment(&self, payment_id: &Uuid) -> Result<Option<Payment>, sqlx::Error>;
}
