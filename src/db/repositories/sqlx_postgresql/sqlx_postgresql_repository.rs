use sqlx::PgPool;
use tracing::{debug, error};
use uuid::Uuid;

use crate::{
    db::{log::LogTypes, PaymentRepository},
    utils::encryption::encrypt_string,
};

#[derive(Clone)]
pub struct SqlxPostgresqlRepository {
    pool: PgPool,
}

impl PaymentRepository for SqlxPostgresqlRepository {
    async fn new() -> Self {
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

        debug!("[DB] Connecting to {}", url);
        let pool = PgPool::connect(&url).await.unwrap();
        debug!("[DB] Connected to {}", url);

        Self { pool }
    }

    async fn add_log(
        &self,
        account_id: &Uuid,
        log_type: LogTypes,
        log_data: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        debug!(
            "[DB] Adding log {} {:?} {:?}",
            account_id, log_type, log_data
        );
        let (log_data, encryption_method) = match log_data {
            Some(log_data) => {
                let (log_data, encryption_method) = encrypt_string(log_data);
                (Some(log_data), Some(encryption_method as i16))
            }
            None => (None, None),
        };
        let log_type: &str = log_type.into();
        let res = sqlx::query!(
            r#"INSERT INTO logs (account_id, action, data, encryption_method) VALUES ($1, $2, $3, $4);"#,
            account_id,
            log_type,
            log_data,
            encryption_method
        )
        .execute(&self.pool)
        .await;

        if let Err(e) = res {
            error!(
                "[DB] Failed to add log {} {:?} {:?}",
                account_id, log_type, log_data
            );
            return Err(e);
        }

        debug!("[DB] Added log to account {}", account_id);

        Ok(())
    }
}
