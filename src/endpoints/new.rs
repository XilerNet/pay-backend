use bitcoincore_rpc::{json::AddressType, Client, RpcApi};
use poem_openapi::Object;
use poem_openapi::{payload::Json, ApiResponse};
use tracing::error;
use uuid::Uuid;

use crate::db::{PaymentRepository, Repository};
use crate::responses::error::ErrorResponse;
use crate::{CHAIN, DOMAIN_PRICE_BTC};

#[derive(Debug, Object, Clone, Eq, PartialEq)]
pub struct CreatePaymentData {
    domains: Vec<String>,
}

#[derive(Debug, Object, Clone, PartialEq)]
pub struct CreatePaymentResponseObject {
    id: Uuid,
    address: String,
    amount: f64,
}

#[derive(ApiResponse)]
pub enum CreatePaymentResponse {
    #[oai(status = 200)]
    Ok(Json<CreatePaymentResponseObject>),

    #[oai(status = 401)]
    Unauthorized(Json<ErrorResponse>),

    #[oai(status = 500)]
    InternalServerError(Json<ErrorResponse>),
}

pub async fn new(
    pool: &Repository,
    rpc: &Client,
    user: &Uuid,
    data: &CreatePaymentData,
) -> CreatePaymentResponse {
    let address = rpc
        .get_new_address(None, Some(AddressType::Bech32m))
        .unwrap()
        .require_network(CHAIN.network())
        .unwrap()
        .to_string();

    let amount = data.domains.len() as f64 * DOMAIN_PRICE_BTC;

    let id = pool.create_payment(user, &address, amount).await;

    // TODO: Generate domain inscriptions and put them in the database
    // TODO: Generate private key for each domain
    // let log = pool.add_log(user, LogTypes::PaymentRequested, log_data)

    match id {
        Ok(id) => CreatePaymentResponse::Ok(Json(CreatePaymentResponseObject {
            id,
            address,
            amount,
        })),
        Err(e) => {
            error!("Failed to create payment: {}", e);
            CreatePaymentResponse::InternalServerError(Json("Internal server error".into()))
        }
    }
}
