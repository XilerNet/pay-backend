use poem_openapi::{payload::Json, ApiResponse};
use tracing::error;
use uuid::Uuid;

use crate::db::repositories::models::payment::Payment;
use crate::db::{PaymentRepository, Repository};
use crate::responses::error::ErrorResponse;

#[derive(ApiResponse)]
pub enum PaymentStatusResponse {
    #[oai(status = 200)]
    Ok(Json<Payment>),

    #[oai(status = 404)]
    NotFound(Json<ErrorResponse>),

    #[oai(status = 500)]
    InternalServerError(Json<ErrorResponse>),
}

pub async fn status(pool: &Repository, user: &Uuid, payment_id: &Uuid) -> PaymentStatusResponse {
    let payment = pool.get_payment(payment_id).await;

    match payment {
        Ok(Some(payment)) => {
            if payment.account_id != *user {
                PaymentStatusResponse::NotFound(Json("Not found".into()))
            } else {
                PaymentStatusResponse::Ok(Json(payment))
            }
        }
        Ok(None) => PaymentStatusResponse::NotFound(Json("Not found".into())),
        Err(e) => {
            error!("Error getting payment: {}", e);
            PaymentStatusResponse::InternalServerError(Json("Internal server error".into()))
        }
    }
}
