use uuid::Uuid;

use crate::db::{log::LogTypes, repositories::models::payment::Payment};

pub struct LoyaltyDiscount(pub String, pub f64, pub String, pub String, pub bool);

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

    async fn add_payment_received(
        &self,
        payment_id: &Uuid,
        received: f64,
        transaction_id: &str,
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
    ) -> Result<Uuid, sqlx::Error>;

    async fn initiate_payment(&self, payment_id: &Uuid) -> Result<(), sqlx::Error>;

    async fn get_to_be_initiated_addresses(&self) -> Result<Vec<String>, sqlx::Error>;

    async fn get_to_be_completed_payments(&self) -> Result<Vec<Uuid>, sqlx::Error>;

    async fn get_payment(&self, payment_id: &Uuid) -> Result<Option<Payment>, sqlx::Error>;

    async fn is_already_processed(
        &self,
        transaction_id: &str,
        address: &str,
    ) -> Result<bool, sqlx::Error>;

    async fn get_payment_by_address(&self, address: &str) -> Result<Option<Payment>, sqlx::Error>;

    async fn add_payment_inscription_contents(
        &self,
        payment_id: &Uuid,
        target: &str,
        contents: &str,
    ) -> Result<(), sqlx::Error>;

    async fn get_payment_inscriptions_content(
        &self,
        payment_id: &Uuid,
    ) -> Result<Option<Vec<(String, String)>>, sqlx::Error>;

    async fn add_private_key(
        &self,
        account_id: &Uuid,
        payment_inscription_content_id: &Uuid,
        domain: &str,
        private_key: &str,
    ) -> Result<(), sqlx::Error>;

    async fn get_owned_domains(
        &self,
        account_id: &Uuid,
    ) -> Result<Vec<(String, bool, Option<String>)>, sqlx::Error>;

    async fn get_already_owned_domains(
        &self,
        user: &Uuid,
        domains: &[String],
    ) -> Result<Vec<String>, sqlx::Error>;

    async fn cleanup_old_orders(&self) -> Result<(), sqlx::Error>;

    async fn get_loyalty_discounts_for_collections(
        &self,
        collections: &[(String, i16, f64)],
    ) -> Result<Vec<LoyaltyDiscount>, sqlx::Error>;

    async fn delete_payment(
        &self,
        user: &Uuid,
        payment_id: &Uuid,
    ) -> Result<Result<(), ()>, sqlx::Error>;

    async fn get_addresses(&self, account_id: &Uuid) -> Result<Vec<String>, sqlx::Error>;
}
