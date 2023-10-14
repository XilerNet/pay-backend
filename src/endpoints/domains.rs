use poem_openapi::Object;
use poem_openapi::{payload::Json, ApiResponse};
use tracing::error;
use uuid::Uuid;

use crate::db::{PaymentRepository, Repository};
use crate::responses::error::ErrorResponse;

#[derive(Debug, Object, Clone, Eq, PartialEq)]
pub struct PaidDomain {
    domain: String,
    payment_completed: bool,
    reveal_tx: Option<String>,
}

#[derive(Debug, Object, Clone, Eq, PartialEq)]
pub struct PaidDomainsResponseObject {
    domains: Vec<PaidDomain>,
}

#[derive(ApiResponse)]
pub enum PaidDomains {
    #[oai(status = 200)]
    Ok(Json<PaidDomainsResponseObject>),

    #[oai(status = 404)]
    NotFound(Json<ErrorResponse>),

    #[oai(status = 500)]
    InternalServerError(Json<ErrorResponse>),
}

pub async fn domains(pool: &Repository, user: &Uuid) -> PaidDomains {
    let domains = pool.get_owned_domains(user).await;

    match domains {
        Ok(domains) => {
            let domains = domains
                .into_iter()
                .map(|domain| PaidDomain {
                    domain: domain.0,
                    payment_completed: domain.1,
                    reveal_tx: domain.2,
                })
                .collect::<Vec<_>>();

            PaidDomains::Ok(Json(PaidDomainsResponseObject { domains }))
        }
        Err(e) => {
            error!("Error getting domains: {}", e);
            PaidDomains::InternalServerError(Json("Internal server error".into()))
        }
    }
}
