use std::str::FromStr;

use bitcoincore_rpc::bitcoin::Address;
use bitcoincore_rpc::{json::AddressType, Client, RpcApi};
use poem_openapi::Object;
use poem_openapi::{payload::Json, ApiResponse};
use pqcrypto_traits::sign::{PublicKey, SecretKey};
use tracing::error;
use uuid::Uuid;

use crate::db::log::LogTypes;
use crate::db::traits::repository::LoyaltyDiscount;
use crate::db::{PaymentRepository, Repository};
use crate::responses::error::ErrorResponse;
use crate::{CHAIN, DOMAIN_PRICE_BTC, MINIMUM_DOMAIN_PRICE_BTC};

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

#[derive(Debug, Object, Clone, Eq, PartialEq)]
pub struct LoyaltyDiscountResponse {
    message: String,
    stackable: bool,
}

#[derive(Debug, Object, Clone, PartialEq)]
pub struct CreatePaymentResponseObject {
    id: Uuid,
    address: String,
    amount: f64,
    loyalty_discounts: Vec<LoyaltyDiscountResponse>,
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

    let already_owned = pool
        .get_already_owned_domains(
            &user,
            &domains.iter().map(|d| d.domain.clone()).collect::<Vec<_>>(),
        )
        .await;

    match already_owned {
        Ok(already_owned) => {
            if already_owned.len() > 0 {
                return CreatePaymentResponse::BadRequest(Json(
                    format!(
                        "Front runner protection: the following domains are already owned or being proccessed (try again later ~35 mins max): {}",
                        already_owned.join(", ")
                    )
                    .as_str()
                    .into(),
                ));
            }
        }
        Err(e) => {
            error!("Failed to run front runner protection check: {}", e);
            return CreatePaymentResponse::InternalServerError(Json(
                "Internal server error".into(),
            ));
        }
    }

    let mut domains_total_price = domains.len() as f64 * DOMAIN_PRICE_BTC;

    let user_brc20_collections = vec![("$BIT".to_string(), 27000)];
    let user_collections = vec![
        ("bit-apes".to_string(), 1),
        ("bitcoin-frogs".to_string(), 1),
        ("other".to_string(), 1),
    ];

    let mut user_collection_query = Vec::new();
    user_collection_query.extend(user_brc20_collections.into_iter().map(|c| (c.0, 1, c.1)));
    user_collection_query.extend(user_collections.into_iter().map(|c| (c.0, 2, c.1)));

    let loyalty_discounts = pool
        .get_loyalty_discounts_for_collections(&user_collection_query)
        .await
        .unwrap_or_default();

    let mut loyalty_discount_percentage = 0.0;
    let mut loyalty_discount_value = 0.0;
    let mut stackable_loyalty_discount_percentage = 0.0;
    let mut stackable_loyalty_discount_value = 0.0;
    let mut to_add_loyalty_discounts = Vec::new();

    for LoyaltyDiscount(collection_id, amount, currency, message, stackable) in
        loyalty_discounts.iter()
    {
        if !*stackable {
            to_add_loyalty_discounts.push(LoyaltyDiscount(
                collection_id.clone(),
                *amount,
                currency.clone(),
                message.clone(),
                *stackable,
            ));
            continue;
        }

        match currency.as_str() {
            "%" => stackable_loyalty_discount_percentage += *amount,
            "BTC" => stackable_loyalty_discount_value += *amount,
            _ => {
                error!(
                    "LoyaltyDiscount - Invalid currency: {} for {}",
                    currency, collection_id
                );
                return CreatePaymentResponse::InternalServerError(Json(
                    "Invalid loyalty discount, please contact a system administrator.".into(),
                ));
            }
        }
    }

    domains_total_price -= stackable_loyalty_discount_value;

    for LoyaltyDiscount(collection_id, amount, currency, _, _) in to_add_loyalty_discounts.iter() {
        match currency.as_str() {
            "%" => {
                if loyalty_discount_percentage < *amount {
                    loyalty_discount_percentage = *amount;
                }
            }
            "BTC" => {
                if loyalty_discount_value < *amount {
                    loyalty_discount_value = *amount;
                }
            }
            _ => {
                error!(
                    "LoyaltyDiscount - Invalid currency: {} for {}",
                    currency, collection_id
                );
                return CreatePaymentResponse::InternalServerError(Json(
                    "Invalid loyalty discount, please contact a system administrator.".into(),
                ));
            }
        }
    }

    domains_total_price -= loyalty_discount_value;
    let percentage_discount = loyalty_discount_percentage + stackable_loyalty_discount_percentage;
    domains_total_price *= 1f64 - (percentage_discount / 100f64);

    if domains_total_price < MINIMUM_DOMAIN_PRICE_BTC {
        return CreatePaymentResponse::InternalServerError(
            Json("Incorrect calculation of discounts, please contact a system administrator to get this in order.".into()),
        );
    }

    let id = pool
        .create_payment(user, &address, domains_total_price)
        .await;

    match id {
        Ok(id) => {
            for domain in domains.iter() {
                let (inscription, private_key) = generate_domain_inscription(&domain.domain);

                let res = pool
                    .create_payment_inscription(&id, &domain.target, &inscription)
                    .await;

                if res.is_err() {
                    error!(
                        "Failed to create payment inscription: {}",
                        res.err().unwrap()
                    );
                    return CreatePaymentResponse::InternalServerError(Json(
                        "Internal server error".into(),
                    ));
                }

                let id = res.unwrap();

                let res = pool
                    .add_private_key(&user, &id, &domain.domain, &private_key)
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
                amount: domains_total_price,
                loyalty_discounts: loyalty_discounts
                    .into_iter()
                    .map(|dc| LoyaltyDiscountResponse {
                        message: dc.3,
                        stackable: dc.4,
                    })
                    .collect(),
            }))
        }
        Err(e) => {
            error!("Failed to create payment: {}", e);
            CreatePaymentResponse::InternalServerError(Json("Internal server error".into()))
        }
    }
}
