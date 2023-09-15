#![feature(async_fn_in_trait)]
use std::{collections::HashSet, env, sync::Arc};

use bitcoin::chain::Chain;
use bitcoincore_rpc::{json::AddressType, Auth, Client, RpcApi};
use db::{traits::SessionRepository, PaymentRepository, Repository};
use endpoints::new::{CreatePaymentData, CreatePaymentResponse};
use poem::{
    listener::TcpListener, middleware::Cors, web::Data, EndpointExt, Request, Route, Server,
};
use poem_openapi::{auth::Bearer, payload::Json, OpenApi, OpenApiService, SecurityScheme};
use std::ops::Deref;
use uuid::Uuid;

pub mod bitcoin;
pub mod db;
pub mod endpoints;
pub mod responses;
pub mod utils;

pub const CHAIN: Chain = Chain::Testnet;
pub const DOMAIN_PRICE_BTC: f64 = 0.0005;
const BITCOIN_WALLET_NAME: &str = "ord";
const COOKIE_LOCATION: &str = "/run/media/arthur/T7/bitcoin/testnet3/.cookie";

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

    // TODO: Status endpoint
}

// TODO: Make background task to check for payments and update those states
fn mmain() {
    let rpc_url = format!(
        "http://localhost:{}/wallet/{}",
        CHAIN.default_rpc_port(),
        BITCOIN_WALLET_NAME
    );
    let rpc = Client::new(&rpc_url, Auth::CookieFile(COOKIE_LOCATION.into())).unwrap();

    // get new wallet address
    let address = rpc
        .get_new_address(None, Some(AddressType::Bech32m))
        .unwrap()
        .require_network(CHAIN.network())
        .unwrap();
    println!("New address: {}", address);
    println!("Waiting for UTXO's...");

    let mut received_transactions = HashSet::new();
    let mut total_received = 0.0;

    loop {
        let utxos = rpc
            .list_unspent(Some(0), Some(999_999), Some(&[&address]), Some(true), None)
            .unwrap();

        if utxos.len() > 0 {
            for utxo in &utxos {
                if !received_transactions.contains(&utxo.txid) {
                    println!(
                        "txid: {}, vout: {}, amount: {}, confirmations: {}",
                        utxo.txid, utxo.vout, utxo.amount, utxo.confirmations
                    );
                    total_received += utxo.amount.to_btc();
                    received_transactions.insert(utxo.txid);
                    println!("Total received: {}", total_received);
                }
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    color_eyre::install().ok();
    dotenv::dotenv().ok();
    tracing_subscriber::fmt::init();

    let repository = Repository::new().await;

    let rpc_url = format!(
        "http://localhost:{}/wallet/{}",
        CHAIN.default_rpc_port(),
        BITCOIN_WALLET_NAME
    );
    let rpc = Client::new(&rpc_url, Auth::CookieFile(COOKIE_LOCATION.into())).unwrap();

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
        vec![env::var("PROD_MAIN_URL").expect("PROD_MAIN_URL not set")]
    };

    let routes = Route::new()
        .nest("/", api_service)
        .nest("/swagger", open_api)
        .with(Cors::new().allow_origins(origins))
        .data(repository)
        .data(rpc);

    Server::new(TcpListener::bind("127.0.0.1:25202"))
        .run(routes)
        .await?;

    Ok(())
}
