use std::str::FromStr;

use bitcoincore_rpc::bitcoin::Address;
use bitcoincore_rpc::{json::AddressType, Client, RpcApi};
use poem_openapi::Object;
use poem_openapi::{payload::Json, ApiResponse};
use pqcrypto_traits::sign::{PublicKey, SecretKey};
use tracing::error;
use uuid::Uuid;

use crate::db::log::LogTypes;
use crate::db::{PaymentRepository, Repository};
use crate::responses::error::ErrorResponse;
use crate::{CHAIN, DOMAIN_PRICE_BTC};

const DOMAIN_REGEX: &str = r"^[a-z\d](?:[a-z\d-]{0,251}[a-z\d])?\.?o?$";

#[derive(Debug, Object, Clone, Eq, PartialEq)]
pub struct CreatePaymentDataDomain {
    domain: String,
    target: String,
}

#[derive(Debug, Object, Clone, Eq, PartialEq)]
pub struct CreatePaymentData {
    domains: Vec<CreatePaymentDataDomain>,
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

    #[oai(status = 400)]
    BadRequest(Json<ErrorResponse>),

    #[oai(status = 401)]
    Unauthorized(Json<ErrorResponse>),

    #[oai(status = 500)]
    InternalServerError(Json<ErrorResponse>),
}

fn generate_domain_inscription(domain: &str) -> (String, String) {
    let (public_key, private_key) = pqcrypto_dilithium::dilithium5_keypair();
    let public_key = hex::encode(public_key.as_bytes());
    let private_key = hex::encode(private_key.as_bytes());

    let current_epoch = chrono::Utc::now().timestamp_millis() / 1000;

    let domain_fmt = format!("DOMAIN {domain} {current_epoch}");
    let validity_fmt = format!("DOMAIN-VALIDITY {domain} dilithium5 {public_key}");
    let signature_fmt = "null null".to_string();

    let inscription = [domain_fmt, validity_fmt, signature_fmt].join("\n");

    (inscription, private_key)
}

pub async fn new(
    pool: &Repository,
    rpc: &Client,
    user: &Uuid,
    data: &CreatePaymentData,
) -> CreatePaymentResponse {
    if data.domains.len() == 0 {
        return CreatePaymentResponse::BadRequest(Json("No domains provided".into()));
    }

    let address = rpc
        .get_new_address(None, Some(AddressType::Bech32m))
        .unwrap()
        .require_network(CHAIN.network())
        .unwrap()
        .to_string();

    let domains = data
        .domains
        .iter()
        .filter(|d| d.domain.len() > 0)
        .map(|d| {
            let domain = if d.domain.ends_with(".o") {
                d.domain.clone()
            } else {
                format!("{}.o", d.domain)
            };

            CreatePaymentDataDomain {
                domain,
                target: d.target.clone(),
            }
        })
        .collect::<Vec<_>>();

    for domain in domains.iter() {
        if !regex::Regex::new(DOMAIN_REGEX)
            .unwrap()
            .is_match(&domain.domain)
        {
            return CreatePaymentResponse::BadRequest(Json(
                format!("Invalid domain: {}", domain.domain).as_str().into(),
            ));
        }

        if let Ok(address) = Address::from_str(&domain.target) {
            if let Err(_) = address.require_network(CHAIN.network()) {
                return CreatePaymentResponse::BadRequest(Json(
                    format!(
                        "Address {} for domain {} is not on the correct network (should be on {})",
                        domain.target,
                        domain.domain,
                        CHAIN.to_string()
                    )
                    .as_str()
                    .into(),
                ));
            }
        } else {
            return CreatePaymentResponse::BadRequest(Json(
                format!(
                    "Invalid target address ({}) for domain: {}",
                    domain.target, domain.domain
                )
                .as_str()
                .into(),
            ));
        }
    }

    let amount = domains.len() as f64 * DOMAIN_PRICE_BTC;

    let id = pool.create_payment(user, &address, amount).await;

    match id {
        Ok(id) => {
            for domain in domains.iter() {
                let (inscription, private_key) = generate_domain_inscription(&domain.domain);

                let res = pool
                    .create_payment_inscription(&id, &domain.target, &inscription)
                    .await;

                if let Err(e) = res {
                    error!("Failed to create payment inscription: {}", e);
                    return CreatePaymentResponse::InternalServerError(Json(
                        "Internal server error".into(),
                    ));
                }

                let res = pool
                    .add_private_key(&user, &domain.domain, &private_key)
                    .await;

                if let Err(e) = res {
                    error!("Failed to create payment private key: {}", e);
                    return CreatePaymentResponse::InternalServerError(Json(
                        "Internal server error".into(),
                    ));
                }
            }

            let log_data = format!("New payment created: {} for {}", id, user);
            let log = pool
                .add_log(user, LogTypes::PaymentRequested, Some(&log_data))
                .await;

            if let Err(e) = log {
                error!("Failed to create payment log: {}", e);
                return CreatePaymentResponse::InternalServerError(Json(
                    "Internal server error".into(),
                ));
            }

            CreatePaymentResponse::Ok(Json(CreatePaymentResponseObject {
                id,
                address,
                amount,
            }))
        }
        Err(e) => {
            error!("Failed to create payment: {}", e);
            CreatePaymentResponse::InternalServerError(Json("Internal server error".into()))
        }
    }
}
