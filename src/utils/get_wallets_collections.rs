use std::collections::HashMap;

use futures::try_join;
use serde::{Deserialize, Serialize};

const BRC_20_API_URL: &str = "https://api.bitcheck.me/get-owned/brc20";
const COLLECTIONS_API_URL: &str = "https://api.bitcheck.me/get-owned/collections";

#[derive(Debug, Clone, Deserialize)]
pub struct Collection {
    pub amount: f64,
    pub ticker: String,
}

impl From<(String, f64)> for Collection {
    fn from(tuple: (String, f64)) -> Self {
        Collection {
            amount: tuple.1,
            ticker: tuple.0.to_uppercase(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Brc20Collection {
    pub amount: f64,
    pub ticker: String,
}

#[derive(Debug, Clone, Serialize)]
struct AddressesRequest {
    addresses: Vec<String>,
}

type CollectionsApiResponse = HashMap<String, f64>;
type CollectionsResponse = Result<Vec<Collection>, reqwest::Error>;

async fn get_from(url: &str, data: &AddressesRequest) -> CollectionsResponse {
    Ok(reqwest::Client::new()
        .post(url)
        .json(data)
        .send()
        .await?
        .json::<CollectionsApiResponse>()
        .await?
        .into_iter()
        .map(Collection::from)
        .collect::<Vec<Collection>>())
}

async fn get_brc20_collections(addresses: &AddressesRequest) -> CollectionsResponse {
    get_from(BRC_20_API_URL, addresses).await
}

async fn get_collections(addresses: &AddressesRequest) -> CollectionsResponse {
    get_from(COLLECTIONS_API_URL, addresses).await
}

#[derive(Debug)]
pub struct WalletCollections {
    pub brc20s: Vec<Collection>,
    pub collections: Vec<Collection>,
}

pub async fn get_wallets_collections(
    addresses: &[&str],
) -> Result<WalletCollections, reqwest::Error> {
    let addresses_request = AddressesRequest {
        addresses: addresses.iter().map(|x| x.to_string()).collect(),
    };

    let (brc20s, collections) = try_join!(
        get_brc20_collections(&addresses_request),
        get_collections(&addresses_request),
    )?;

    Ok(WalletCollections {
        brc20s,
        collections,
    })
}
