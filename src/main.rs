#![feature(async_fn_in_trait)]
use std::{env, str::FromStr, sync::Arc};

use bitcoin::chain::Chain;
use bitcoincore_rpc::{
    bitcoin::{address::NetworkChecked, Address},
    Auth, Client, RpcApi,
};
use db::{traits::SessionRepository, PaymentRepository, Repository};
use endpoints::{
    delete::DeletePaymentResponse,
    domains::PaidDomains,
    get_private_key::GetPrivateKeyResponse,
    new::{CreatePaymentData, CreatePaymentResponse},
    pricing::PricingResponse,
    status::PaymentStatusResponse,
};
use poem::{
    listener::TcpListener, middleware::Cors, web::Data, EndpointExt, Request, Route, Server,
};
use poem_openapi::{
    auth::Bearer,
    param::{Path, Query},
    payload::Json,
    OpenApi, OpenApiService, SecurityScheme,
};
use std::ops::Deref;
use tracing::{error, info};
use uuid::Uuid;

use crate::db::log::LogTypes;

pub mod bitcoin;
pub mod db;
pub mod endpoints;
pub mod responses;
pub mod utils;

#[cfg(debug_assertions)]
pub const CHAIN: Chain = Chain::Regtest;

#[cfg(not(debug_assertions))]
pub const CHAIN: Chain = Chain::Mainnet;

pub const DOMAIN_PRICE_BTC: f64 = 0.0007;
pub const MINIMUM_DOMAIN_PRICE_BTC: f64 = 0.0004;

#[cfg(debug_assertions)]
const BITCOIN_WALLET_NAME: &str = "ord";

#[cfg(not(debug_assertions))]
const BITCOIN_WALLET_NAME: &str = "xiler";
const COOKIE_LOCATION: &str = "/home/bitcheck/.bitcoin/.cookie";
const CONFIRMATIONS_REQUIRED: u32 = 1;

struct ApiKeyContext {
    id: Uuid,
}

#[derive(SecurityScheme)]
#[oai(
    ty = "bearer",
    key_name = "Xiler-Accounts-API-Key",
    key_in = "header",
    checker = "check_api_key"
)]
struct AuthApiKey(ApiKeyContext);

impl Deref for AuthApiKey {
    type Target = ApiKeyContext;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

async fn check_api_key(req: &Request, api_key: Bearer) -> Option<ApiKeyContext> {
    let pool = req.data::<Repository>().unwrap();
    let id = pool.get_session(&api_key.token).await;

    if let Ok(id) = id {
        Some(ApiKeyContext { id })
    } else {
        None
    }
}

struct Api;

#[OpenApi]
impl Api {
    #[oai(path = "/new", method = "post")]
    async fn new(
        &self,
        pool: Data<&Repository>,
        rpc: Data<&Arc<Client>>,
        auth: AuthApiKey,
        data: Json<CreatePaymentData>,
    ) -> CreatePaymentResponse {
        endpoints::new::new(&pool, &rpc, &auth.id, &data).await
    }

    #[oai(path = "/status/:id", method = "get")]
    async fn status(
        &self,
        pool: Data<&Repository>,
        auth: AuthApiKey,
        id: Path<Uuid>,
    ) -> PaymentStatusResponse {
        endpoints::status::status(&pool, &auth.id, &id).await
    }

    #[oai(path = "/delete/:id", method = "delete")]
    async fn delete(
        &self,
        pool: Data<&Repository>,
        auth: AuthApiKey,
        id: Path<Uuid>,
    ) -> DeletePaymentResponse {
        endpoints::delete::delete(&pool, &auth.id, &id).await
    }

    #[oai(path = "/domains", method = "get")]
    async fn domains(&self, pool: Data<&Repository>, auth: AuthApiKey) -> PaidDomains {
        endpoints::domains::domains(&pool, &auth.id).await
    }

    #[oai(path = "/pricing", method = "get")]
    async fn pricing(
        &self,
        pool: Data<&Repository>,
        auth: AuthApiKey,
        amount: Query<u32>,
    ) -> PricingResponse {
        endpoints::pricing::get_price(&pool, &auth.id, amount.0).await
    }

    #[oai(path = "/private-key/:domain", method = "get")]
    async fn private_key(
        &self,
        pool: Data<&Repository>,
        auth: AuthApiKey,
        domain: Path<String>,
    ) -> GetPrivateKeyResponse {
        endpoints::get_private_key::get_private_key(&pool, &auth.id, &domain.0).await
    }
}

fn get_rpc() -> Client {
    let rpc_url = format!(
        "http://localhost:{}/wallet/{}",
        CHAIN.default_rpc_port(),
        BITCOIN_WALLET_NAME
    );

    #[cfg(debug_assertions)]
    let auth = Auth::UserPass("admin1".into(), "123".into()); // for testing purposes

    #[cfg(not(debug_assertions))]
    let auth = Auth::CookieFile(COOKIE_LOCATION.into());

    Client::new(&rpc_url, auth).unwrap()
}

async fn background_payment_processor() {
    info!("Starting background payment processor");
    let rpc = get_rpc();
    let pool = Repository::new().await;
    info!("Connected to Bitcoin RPC and database");

    loop {
        let watch_addresses = pool
            .get_to_be_initiated_addresses()
            .await
            .unwrap()
            .into_iter()
            .map(|address| Address::from_str(&address).unwrap())
            .map(|address| address.require_network(CHAIN.network()))
            .flatten()
            .collect::<Vec<Address<NetworkChecked>>>();

        let utxos = rpc
            .list_unspent(
                Some(0),
                Some(100), // Do not update utxo's with more than 100 confirmations
                Some(watch_addresses.iter().collect::<Vec<_>>().as_slice()),
                Some(true),
                None,
            )
            .unwrap();

        if utxos.len() > 0 {
            for utxo in &utxos {
                let address = match utxo
                    .address
                    .clone()
                    .unwrap()
                    .require_network(CHAIN.network())
                {
                    Ok(address) => address.to_string(),
                    Err(_) => continue,
                };

                let amount = utxo.amount.to_btc();
                let txid = utxo.txid.clone().to_string();
                let confirmations = utxo.confirmations;

                let is_already_processed =
                    pool.is_already_processed(&txid, &address).await.unwrap();

                if is_already_processed {
                    continue;
                }

                let payment = match pool.get_payment_by_address(&address).await.unwrap() {
                    Some(payment_id) => payment_id,
                    None => continue,
                };

                if !payment.initiated {
                    let res = pool.initiate_payment(&payment.id).await;

                    if let Err(e) = res {
                        error!("Error initiating payment: {}", e);
                        continue;
                    }

                    let log_message = format!(
                        "account {}, payment: {} transaction: {}, initiated: ({}BTC)",
                        payment.account_id, payment.id, txid, payment.amount
                    );
                    let res = pool
                        .add_log(
                            &payment.account_id,
                            LogTypes::PaymentReceivedUnconfirmed,
                            Some(&log_message),
                        )
                        .await;

                    if let Err(e) = res {
                        error!("Error adding log: {}", e);
                        continue;
                    }
                }

                if confirmations < CONFIRMATIONS_REQUIRED {
                    continue;
                }

                let res = pool.add_payment_received(&payment.id, amount, &txid).await;

                if let Err(e) = res {
                    error!("Error adding payment received: {}", e);
                    continue;
                }

                info!("Payment {} received {}BTC", payment.id, amount);

                let log_message = format!(
                    "account {}, payment: {} transaction: {}, received {}BTC",
                    payment.account_id, payment.id, txid, amount
                );
                let res = pool
                    .add_log(
                        &payment.account_id,
                        LogTypes::PaymentReceivedConfirmed,
                        Some(&log_message),
                    )
                    .await;

                if let Err(e) = res {
                    error!("Error adding log: {}", e);
                    continue;
                }
            }
        }

        if let Err(e) = pool.cleanup_old_orders().await {
            error!("Error cleaning up old orders: {}", e);
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    color_eyre::install().ok();
    dotenv::dotenv().ok();
    tracing_subscriber::fmt::init();

    let repository = Repository::new().await;

    let rpc = get_rpc();
    if !rpc
        .list_wallets()
        .unwrap()
        .contains(&BITCOIN_WALLET_NAME.to_string())
    {
        rpc.load_wallet(BITCOIN_WALLET_NAME).unwrap();
    }
    let rpc = Arc::new(rpc);

    let api_service = OpenApiService::new(Api, "Xiler Authentication API", "v0.0.1")
        .server("http://localhost:25202")
        .server("https://pay-api.xiler.net");
    let open_api = api_service.swagger_ui();

    let origins: Vec<String> = if cfg!(debug_assertions) {
        vec![
            env::var("DEV_MAIN_URL").expect("DEV_MAIN_URL not set"),
            "http://localhost:25202".to_string(),
        ]
    } else {
        vec![
            env::var("PROD_MAIN_URL").expect("PROD_MAIN_URL not set"),
            "https://www.xiler.net".to_string(),
            "https://xiler.net".to_string(),
        ]
    };

    let routes = Route::new()
        .nest("/", api_service)
        .nest("/swagger", open_api)
        .with(Cors::new().allow_origins(origins))
        .data(repository)
        .data(rpc);

    tokio::spawn(background_payment_processor());

    Server::new(TcpListener::bind("127.0.0.1:25202"))
        .run(routes)
        .await?;

    Ok(())
}
