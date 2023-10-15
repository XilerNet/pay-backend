use sqlx::PgPool;
use tracing::{debug, error};
use uuid::Uuid;

use crate::{
    db::{
        log::LogTypes, repositories::models::payment::Payment, traits::SessionRepository,
        PaymentRepository,
    },
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

    async fn add_payment_received(
        &self,
        payment_id: &Uuid,
        received: f64,
        transaction_id: &str,
    ) -> Result<(), sqlx::Error> {
        debug!(
            "[DB] Adding payment received {} {} {}",
            payment_id, received, transaction_id
        );

        let res = sqlx::query!(
            r#"INSERT INTO payment_transactions (payment_id, transaction_id) VALUES ($1, $2);"#,
            payment_id,
            transaction_id
        );

        let res = res.execute(&self.pool).await;

        if let Err(e) = res {
            error!(
                "[DB] Failed to add payment received {} {} {}",
                payment_id, received, transaction_id
            );
            return Err(e);
        }

        let res = sqlx::query!(
            r#"UPDATE payments SET received = received + $1 WHERE id = $2;"#,
            received,
            payment_id
        );

        let res = res.execute(&self.pool).await;

        if let Err(e) = res {
            error!(
                "[DB] Failed to add payment received {} {} {}",
                payment_id, received, transaction_id
            );
            return Err(e);
        }

        debug!(
            "[DB] Added payment received {} {} {}",
            payment_id, received, transaction_id
        );

        Ok(())
    }

    async fn complete_payment(&self, payment_id: &Uuid) -> Result<(), sqlx::Error> {
        debug!("[DB] Completing payment {}", payment_id);

        let res = sqlx::query!(
            r#"UPDATE payments SET completed = TRUE WHERE id = $1;"#,
            payment_id
        );
        let res = res.execute(&self.pool).await;

        if let Err(e) = res {
            error!("[DB] Failed to complete payment {}", payment_id);
            return Err(e);
        }

        debug!("[DB] Completed payment {}", payment_id);

        Ok(())
    }

    async fn create_payment(
        &self,
        account_id: &Uuid,
        address: &str,
        amount: f64,
    ) -> Result<Uuid, sqlx::Error> {
        debug!("[DB] Creating payment for account {}", account_id);

        let res = sqlx::query!(
            r#"INSERT INTO payments (account_id, address, amount) VALUES ($1, $2, $3) RETURNING id;"#,
            account_id,
            address,
            amount
        );
        let res = res.fetch_one(&self.pool).await;

        if let Err(e) = res {
            error!("[DB] Failed to create payment for account {}", account_id);
            return Err(e);
        }

        let payment_id = res.unwrap().id;

        debug!(
            "[DB] Created payment {} for account {}",
            payment_id, account_id
        );
        Ok(payment_id)
    }

    async fn create_payment_inscription(
        &self,
        payment_id: &Uuid,
        target: &str,
        contents: &str,
    ) -> Result<Uuid, sqlx::Error> {
        debug!(
            "[DB] Creating payment inscription {} {} {}",
            payment_id, target, contents
        );

        let res = sqlx::query!(
            r#"INSERT INTO payment_inscription_contents (payment_id, target, content) VALUES ($1, $2, $3) RETURNING id;"#,
            payment_id,
            target,
            contents
        );

        match res.fetch_one(&self.pool).await {
            Ok(res) => {
                debug!(
                    "[DB] Created payment inscription {} {} {}",
                    payment_id, target, contents
                );

                Ok(res.id)
            }
            Err(e) => {
                error!(
                    "[DB] Failed to create payment inscription {} {} {}",
                    payment_id, target, contents
                );
                return Err(e);
            }
        }
    }

    async fn initiate_payment(&self, payment_id: &Uuid) -> Result<(), sqlx::Error> {
        debug!("[DB] Initiating payment {}", payment_id);

        let res = sqlx::query!(
            r#"UPDATE payments SET initiated = TRUE WHERE id = $1;"#,
            payment_id
        );
        let res = res.execute(&self.pool).await;

        if let Err(e) = res {
            error!("[DB] Failed to initiate payment {}", payment_id);
            return Err(e);
        }

        debug!("[DB] Initiated payment {}", payment_id);

        Ok(())
    }

    async fn get_to_be_initiated_addresses(&self) -> Result<Vec<String>, sqlx::Error> {
        debug!("[DB] Getting to be initiated payments");

        let res = sqlx::query!(r#"SELECT address FROM payments WHERE initiated = FALSE;"#)
            .fetch_all(&self.pool)
            .await;

        if let Err(e) = res {
            error!("[DB] Failed to get to be initiated payments");
            return Err(e);
        }

        let res = res.unwrap();

        let mut payments = Vec::new();

        for row in res {
            payments.push(row.address);
        }

        debug!("[DB] Got to be initiated payments {:?}", payments);

        Ok(payments)
    }

    async fn get_to_be_completed_payments(&self) -> Result<Vec<Uuid>, sqlx::Error> {
        debug!("[DB] Getting to be completed payments");

        let res = sqlx::query!( r#"SELECT id FROM payments WHERE initiated = TRUE AND completed = FALSE AND received >= amount"#)
        .fetch_all(&self.pool)
        .await;

        if let Err(e) = res {
            error!("[DB] Failed to get to be completed payments");
            return Err(e);
        }

        let res = res.unwrap();

        let mut payments = Vec::new();

        for row in res {
            payments.push(row.id);
        }

        debug!("[DB] Got to be completed payments {:?}", payments);

        Ok(payments)
    }

    async fn get_payment(&self, payment_id: &Uuid) -> Result<Option<Payment>, sqlx::Error> {
        debug!("[DB] Getting payment {}", payment_id);

        let res = sqlx::query!(r#"SELECT * FROM payments WHERE id = $1;"#, payment_id)
            .fetch_optional(&self.pool)
            .await;

        if let Err(e) = res {
            error!("[DB] Failed to get payment {}", payment_id);
            return Err(e);
        }

        let res = res.unwrap();

        if let Some(row) = res {
            debug!("[DB] Got payment {}", payment_id);
            return Ok(Some(Payment {
                id: row.id,
                account_id: row.account_id,
                address: row.address,
                amount: row.amount,
                received: row.received,
                initiated: row.initiated,
                completed: row.completed,
                created_at: row.created_at,
                updated_at: row.updated_at,
            }));
        }

        debug!("[DB] Payment {} not found", payment_id);

        Ok(None)
    }

    async fn is_already_processed(
        &self,
        transaction_id: &str,
        address: &str,
    ) -> Result<bool, sqlx::Error> {
        debug!(
            "[DB] Checking if transaction {} for address {} is already processed",
            transaction_id, address
        );

        let res = sqlx::query!(
            r#"SELECT * FROM payment_transactions WHERE transaction_id = $1 AND payment_id IN (SELECT id FROM payments WHERE address = $2);"#,
            transaction_id,
            address
        )
        .fetch_optional(&self.pool)
        .await;

        if let Err(e) = res {
            error!(
                "[DB] Failed to check if transaction {} for address {} is already processed",
                transaction_id, address
            );
            return Err(e);
        }

        let res = res.unwrap();
        if let Some(_) = res {
            debug!(
                "[DB] Transaction {} for address {} is already processed",
                transaction_id, address
            );
            return Ok(true);
        }

        debug!(
            "[DB] Transaction {} for address {} is not processed yet",
            transaction_id, address
        );
        return Ok(false);
    }

    async fn get_payment_by_address(&self, address: &str) -> Result<Option<Payment>, sqlx::Error> {
        debug!("[DB] Getting payment by address {}", address);

        let res = sqlx::query!(r#"SELECT * FROM payments WHERE address = $1;"#, address)
            .fetch_optional(&self.pool)
            .await;

        if let Err(e) = res {
            error!("[DB] Failed to get payment by address {}", address);
            return Err(e);
        }

        let res = res.unwrap();

        if let Some(row) = res {
            debug!("[DB] Got payment by address {}", address);
            return Ok(Some(Payment {
                id: row.id,
                account_id: row.account_id,
                address: row.address,
                amount: row.amount,
                received: row.received,
                initiated: row.initiated,
                completed: row.completed,
                created_at: row.created_at,
                updated_at: row.updated_at,
            }));
        }

        debug!("[DB] Payment by address {} not found", address);

        Ok(None)
    }

    async fn add_payment_inscription_contents(
        &self,
        payment_id: &Uuid,
        target: &str,
        contents: &str,
    ) -> Result<(), sqlx::Error> {
        debug!(
            "[DB] Adding payment inscription contents for payment {}",
            payment_id
        );

        let res = sqlx::query!(
            r#"INSERT INTO payment_inscription_contents (payment_id, target, content) VALUES ($1, $2, $3);"#,
            payment_id,
            target,
            contents
        )
        .execute(&self.pool)
        .await;

        if let Err(e) = res {
            error!(
                "[DB] Failed to add payment inscription contents for payment {}",
                payment_id
            );
            return Err(e);
        }

        debug!(
            "[DB] Added payment inscription contents for payment {}",
            payment_id
        );

        Ok(())
    }

    async fn get_payment_inscriptions_content(
        &self,
        payment_id: &Uuid,
    ) -> Result<Option<Vec<(String, String)>>, sqlx::Error> {
        debug!(
            "[DB] Getting payment inscription contents for payment {}",
            payment_id
        );

        let res = sqlx::query!(
            r#"SELECT target, content FROM payment_inscription_contents WHERE payment_id = $1;"#,
            payment_id
        )
        .fetch_all(&self.pool)
        .await;

        if let Err(e) = res {
            error!(
                "[DB] Failed to get payment inscription contents for payment {}",
                payment_id
            );
            return Err(e);
        }

        let res = res.unwrap();

        let mut contents = Vec::new();

        for row in res {
            contents.push((row.target, row.content));
        }

        debug!(
            "[DB] Got payment inscription contents for payment {}",
            payment_id
        );

        Ok(Some(contents))
    }

    async fn add_private_key(
        &self,
        account_id: &Uuid,
        payment_inscription_content_id: &Uuid,
        domain: &str,
        private_key: &str,
    ) -> Result<(), sqlx::Error> {
        debug!(
            "[DB] Adding private key for account {} and domain {}",
            account_id, domain
        );

        let (private_key, encryption_method) = encrypt_string(private_key);

        let res = sqlx::query!(
            r#"INSERT INTO private_keys (account_id, payment_inscription_content_id, domain, encryption_method, private_key) VALUES ($1, $2, $3, $4, $5);"#,
            account_id,
            payment_inscription_content_id,
            domain,
            encryption_method as i16,
            private_key
        )
        .execute(&self.pool)
        .await;

        if let Err(e) = res {
            error!(
                "[DB] Failed to add private key for account {} and domain {}",
                account_id, domain
            );
            return Err(e);
        }

        debug!(
            "[DB] Added private key for account {} and domain {}",
            account_id, domain
        );

        Ok(())
    }

    async fn get_owned_domains(
        &self,
        account_id: &Uuid,
    ) -> Result<Vec<(String, bool, Option<String>)>, sqlx::Error> {
        debug!("[DB] Getting owned domains for account {}", account_id);

        let res = sqlx::query!(
            r#"SELECT private_keys.domain, payments.completed, payment_inscriptions.reveal_tx as "reveal_tx?" 
            FROM private_keys 
            INNER JOIN payment_inscription_contents ON payment_inscription_contents.id = private_keys.payment_inscription_content_id 
            INNER JOIN payments ON payments.id = payment_inscription_contents.payment_id 
            LEFT JOIN payment_inscriptions ON payment_inscriptions.content = payment_inscription_contents.id 
                WHERE payments.initiated = TRUE 
                AND private_keys.account_id = $1;"#,
            account_id
        )
        .fetch_all(&self.pool)
        .await;

        if let Err(e) = res {
            error!(
                "[DB] Failed to get owned domains for account {}",
                account_id
            );
            return Err(e);
        }

        let res = res.unwrap();

        let mut domains = Vec::new();

        for row in res {
            domains.push((row.domain, row.completed, row.reveal_tx));
        }

        debug!("[DB] Got owned domains for account {}", account_id);

        Ok(domains)
    }

    async fn get_already_owned_domains(
        &self,
        domains: &[String],
    ) -> Result<Vec<String>, sqlx::Error> {
        debug!("[DB] Getting already owned domains {:?}", domains);

        let res = sqlx::query!(
            r#"SELECT private_keys.domain FROM private_keys 
            INNER JOIN payment_inscription_contents ON payment_inscription_contents.id = private_keys.payment_inscription_content_id
            INNER JOIN payments ON payments.id = payment_inscription_contents.payment_id
            WHERE private_keys.domain = ANY($1)
                AND payments.initiated = True;"#,
            domains
        )
        .fetch_all(&self.pool)
        .await;

        if let Err(e) = res {
            error!("[DB] Failed to get already owned domains {:?}", domains);
            return Err(e);
        }

        let res = res.unwrap();

        let mut domains = Vec::new();

        for row in res {
            domains.push(row.domain);
        }

        debug!("[DB] Got already owned domains {:?}", domains);

        Ok(domains)
    }
}

impl SessionRepository for SqlxPostgresqlRepository {
    async fn get_session(&self, token: &str) -> Result<Uuid, sqlx::Error> {
        debug!("[DB] Getting session {}", token);
        let id = sqlx::query!(r#"SELECT account_id FROM sessions WHERE id = $1;"#, token)
            .fetch_one(&self.pool)
            .await?;

        debug!("[DB] Got session {} from token {}", id.account_id, token);

        Ok(id.account_id)
    }
}
