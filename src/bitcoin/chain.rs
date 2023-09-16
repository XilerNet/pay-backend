use std::str::FromStr;

use bitcoincore_rpc::bitcoin::Network;

pub enum Chain {
    Mainnet,
    Testnet,
}

impl Chain {
    pub(crate) fn default_rpc_port(self) -> u16 {
        match self {
            Self::Mainnet => 8332,
            Self::Testnet => 18332,
        }
    }

    pub(crate) fn network(self) -> Network {
        match self {
            Self::Mainnet => Network::Bitcoin,
            Self::Testnet => Network::Testnet,
        }
    }
}

impl FromStr for Chain {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "mainnet" => Ok(Self::Mainnet),
            "testnet" => Ok(Self::Testnet),
            _ => Err(format!("unknown chain: {}", s)),
        }
    }
}

impl ToString for Chain {
    fn to_string(&self) -> String {
        match self {
            Self::Mainnet => "mainnet".to_string(),
            Self::Testnet => "testnet".to_string(),
        }
    }
}
