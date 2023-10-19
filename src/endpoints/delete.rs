use poem_openapi::payload::PlainText;
use poem_openapi::{payload::Json, ApiResponse};
use tracing::error;
use uuid::Uuid;

use crate::db::{PaymentRepository, Repository};
use crate::responses::error::ErrorResponse;

#[derive(ApiResponse)]
pub enum DeletePaymentResponse {
    #[oai(status = 200)]
    Ok(PlainText<String>),

    #[oai(status = 404)]
    NotFound(Json<ErrorResponse>),

    #[oai(status = 500)]
    InternalServerError(Json<ErrorResponse>),
}

pub async fn delete(pool: &Repository, user: &Uuid, payment_id: &Uuid) -> DeletePaymentResponse {
    let payment = pool.delete_payment(&user, &payment_id).await;

    match payment {
        Ok(Ok(())) => DeletePaymentResponse::Ok(PlainText("ok".to_string())),
        Ok(Err(())) => DeletePaymentResponse::NotFound(Json("Not found".into())),
        Err(e) => {
            error!("Error getting payment: {}", e);
            DeletePaymentResponse::InternalServerError(Json("Internal server error".into()))
        }
    }
}
