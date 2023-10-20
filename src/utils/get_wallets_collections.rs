use futures::future::try_join_all;
use serde::Deserialize;

const BRC_20_API_URL: &str = "https://turbo.ordinalswallet.com/wallet/";

#[derive(Debug, Clone, Deserialize)]
pub struct Brc20Collection {
    #[serde(rename = "overall_balance")]
    pub amount: f64,
    pub ticker: String,
}

#[derive(Debug, Clone, Deserialize)]
struct Brc20CollectionResponse {
    #[serde(rename = "brc20")]
    pub brc20s: Vec<Brc20Collection>,
}

async fn get_brc20_collections_by_address(
    address: &str,
) -> Result<Vec<Brc20Collection>, reqwest::Error> {
    let res = reqwest::get(format!("{}{}", BRC_20_API_URL, address))
        .await?
        .json::<Brc20CollectionResponse>()
        .await?;

    Ok(res.brc20s)
}

async fn get_brc20_collections(addresses: &[&str]) -> Result<Vec<Brc20Collection>, reqwest::Error> {
    let futures = addresses
        .iter()
        .map(|address| get_brc20_collections_by_address(address));

    Ok(try_join_all(futures)
        .await?
        .into_iter()
        .flatten()
        .collect::<Vec<Brc20Collection>>())
}

#[derive(Debug)]
pub struct WalletCollections {
    pub brc20s: Vec<Brc20Collection>,
}

pub async fn get_wallets_collections(
    addresses: &[&str],
) -> Result<WalletCollections, reqwest::Error> {
    let brc20s = get_brc20_collections(addresses).await?;

    Ok(WalletCollections { brc20s })
}
