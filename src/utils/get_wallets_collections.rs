use std::collections::HashMap;

use futures::future::try_join_all;
use serde::{Deserialize, Serialize};

const BRC_20_API_URL: &str = "https://turbo.ordinalswallet.com/wallet/";
const COLLECTIONS_API_URL: &str = "https://api.bitcheck.me/get-owned";

#[derive(Debug, Clone, Deserialize)]
pub struct Collection {
    #[serde(rename = "overall_balance")]
    pub amount: f64,
    pub ticker: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Brc20Collection {
    pub amount: f64,
    pub ticker: String,
}

#[derive(Debug, Clone, Deserialize)]
struct Brc20CollectionResponse {
    #[serde(rename = "brc20")]
    pub brc20s: Vec<Collection>,
}

#[derive(Debug, Clone, Serialize)]
struct AddressesRequest {
    addresses: Vec<String>,
}

async fn get_brc20_collections_by_address(
    address: &str,
) -> Result<Vec<Collection>, reqwest::Error> {
    let res = reqwest::get(format!("{}{}", BRC_20_API_URL, address))
        .await?
        .json::<Brc20CollectionResponse>()
        .await?;

    Ok(res.brc20s)
}

async fn get_brc20_collections(addresses: &[&str]) -> Result<Vec<Collection>, reqwest::Error> {
    let futures = addresses
        .iter()
        .map(|address| get_brc20_collections_by_address(address));

    Ok(try_join_all(futures)
        .await?
        .into_iter()
        .flatten()
        .collect::<Vec<Collection>>())
}

async fn get_collections(data: &AddressesRequest) -> Result<Vec<Collection>, reqwest::Error> {
    Ok(reqwest::Client::new()
        .post(COLLECTIONS_API_URL)
        .json(data)
        .send()
        .await?
        .json::<HashMap<String, f64>>()
        .await?
        .into_iter()
        .map(|(k, v)| Collection {
            amount: v,
            ticker: k,
        })
        .collect::<Vec<Collection>>())
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

    let brc20s = get_brc20_collections(addresses).await?;
    let collections = get_collections(&addresses_request).await?;

    Ok(WalletCollections {
        brc20s,
        collections,
    })
}
