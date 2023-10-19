use poem_openapi::Object;
use poem_openapi::{payload::Json, ApiResponse};
use tracing::error;
use uuid::Uuid;

use crate::db::traits::repository::LoyaltyDiscount;
use crate::db::{PaymentRepository, Repository};
use crate::responses::error::ErrorResponse;
use crate::{DOMAIN_PRICE_BTC, MINIMUM_DOMAIN_PRICE_BTC};

#[derive(Debug, Object, Clone, PartialEq)]
pub struct LoyaltyDiscountResponseObject {
    message: String,
    amount: f64,
    currency: String,
}

#[derive(Debug, Object, Clone, PartialEq)]
pub struct PricingResponseObject {
    stackable_loyalty_discounts: Vec<LoyaltyDiscountResponseObject>,

    non_stackable_loyalty_discounts: Vec<String>,
    non_stackable_loyalty_discount: f64,
    non_stackable_loyalty_discount_currency: String,

    final_price: f64,
}

#[derive(ApiResponse)]
pub enum PricingResponse {
    #[oai(status = 200)]
    Ok(Json<PricingResponseObject>),

    #[oai(status = 400)]
    BadRequest(Json<ErrorResponse>),

    #[oai(status = 401)]
    Unauthorized(Json<ErrorResponse>),

    #[oai(status = 500)]
    InternalServerError(Json<ErrorResponse>),
}

pub async fn get_price(pool: &Repository, user: &Uuid, amount: u32) -> PricingResponse {
    if amount == 0 {
        return PricingResponse::BadRequest(Json("Can not calculate price for 0 domains.".into()));
    }

    let mut final_price = amount as f64 * DOMAIN_PRICE_BTC;

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

    let mut stackable_loyalty_discounts: Vec<LoyaltyDiscountResponseObject> = Vec::new();
    let mut stackable_loyalty_discount = 0.0;
    let mut non_stackable_loyalty_discounts: Vec<String> = Vec::new();
    let mut non_stackable_price = final_price;
    let mut non_stackable_loyalty_discount = 0.0;
    let mut non_stackable_loyalty_discount_currency = "".to_string();

    for LoyaltyDiscount(collection_id, amount, currency, message, stackable) in
        loyalty_discounts.iter()
    {
        if *stackable {
            stackable_loyalty_discounts.push(LoyaltyDiscountResponseObject {
                message: message.clone(),
                amount: *amount,
                currency: currency.clone(),
            });
            stackable_loyalty_discount += *amount;
            continue;
        }

        let price_after_discount = match currency.as_str() {
            "%" => final_price * (1f64 - (*amount / 100f64)),
            "BTC" => final_price - *amount,
            _ => {
                error!(
                    "LoyaltyDiscount - Invalid currency: {} for {}",
                    currency, collection_id
                );
                return PricingResponse::InternalServerError(Json(
                    "Invalid loyalty discount, please contact a system administrator.".into(),
                ));
            }
        };

        if price_after_discount < non_stackable_price {
            non_stackable_price = price_after_discount;
            non_stackable_loyalty_discount = *amount;
            non_stackable_loyalty_discount_currency = currency.clone();
        }

        non_stackable_loyalty_discounts.push(message.clone());
    }

    match non_stackable_loyalty_discount_currency.as_str() {
        "%" => stackable_loyalty_discount += non_stackable_loyalty_discount,
        "BTC" => final_price -= non_stackable_loyalty_discount,
        _ => panic!("Impossible situation"),
    };

    final_price *= 1f64 - (stackable_loyalty_discount / 100f64);
    final_price = (final_price * 10000000f64).ceil() / 10000000f64;

    if final_price < MINIMUM_DOMAIN_PRICE_BTC {
        return PricingResponse::InternalServerError(
            Json("Incorrect calculation of discounts, please contact a system administrator to get this in order.".into()),
        );
    }

    PricingResponse::Ok(Json(PricingResponseObject {
        stackable_loyalty_discounts,
        non_stackable_loyalty_discounts,
        non_stackable_loyalty_discount,
        non_stackable_loyalty_discount_currency,
        final_price,
    }))
}
