use poem_openapi::Object;
use poem_openapi::{payload::Json, ApiResponse};
use tracing::error;
use uuid::Uuid;

use crate::db::{PaymentRepository, Repository};
use crate::responses::error::ErrorResponse;

#[derive(Debug, Object, Clone, Eq, PartialEq)]
pub struct GetPrivateKeyResponseObject {
    private_key: String,
}

#[derive(ApiResponse)]
pub enum GetPrivateKeyResponse {
    #[oai(status = 200)]
    Ok(Json<GetPrivateKeyResponseObject>),

    #[oai(status = 404)]
    NotFound(Json<ErrorResponse>),

    #[oai(status = 500)]
    InternalServerError(Json<ErrorResponse>),
}

pub async fn get_private_key(
    pool: &Repository,
    user: &Uuid,
    domain: &str,
) -> GetPrivateKeyResponse {
    match pool.get_private_key(&user, &domain).await {
        Ok(Some(private_key)) => {
            GetPrivateKeyResponse::Ok(Json(GetPrivateKeyResponseObject { private_key }))
        }
        Ok(None) => GetPrivateKeyResponse::NotFound(Json(
            "You do not own this domain or it does not exist!".into(),
        )),
        Err(e) => {
            error!("Received error while fetching private key: {}", e);
            GetPrivateKeyResponse::InternalServerError(Json("Internal server error".into()))
        }
    }
}
