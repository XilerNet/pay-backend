use uuid::Uuid;

use crate::db::log::LogTypes;

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
}
