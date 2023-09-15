#![feature(async_fn_in_trait)]
use std::collections::HashSet;

use bitcoin::chain::Chain;
use bitcoincore_rpc::{json::AddressType, Auth, Client, RpcApi};

pub mod bitcoin;
pub mod db;
pub mod utils;

const CHAIN: Chain = Chain::Testnet;
const BITCOIN_WALLET_NAME: &str = "ord";
const COOKIE_LOCATION: &str = "/run/media/arthur/T7/bitcoin/testnet3/.cookie";

fn main() {
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
