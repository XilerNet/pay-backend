use std::str::FromStr;

use bitcoincore_rpc::bitcoin::Network;

pub enum Chain {
    Mainnet,
    Testnet,
    Regtest,
}

impl Chain {
    pub(crate) fn default_rpc_port(self) -> u16 {
        match self {
            Self::Mainnet => 8332,
            Self::Testnet => 18332,
            Self::Regtest => 19001,
        }
    }

    pub(crate) fn network(self) -> Network {
        match self {
            Self::Mainnet => Network::Bitcoin,
            Self::Testnet => Network::Testnet,
            Self::Regtest => Network::Regtest,
        }
    }
}

impl FromStr for Chain {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "mainnet" => Ok(Self::Mainnet),
            "testnet" => Ok(Self::Testnet),
            "regtest" => Ok(Self::Regtest),
            _ => Err(format!("unknown chain: {}", s)),
        }
    }
}

impl ToString for Chain {
    fn to_string(&self) -> String {
        match self {
            Self::Mainnet => "mainnet".to_string(),
            Self::Testnet => "testnet".to_string(),
            Self::Regtest => "regtest".to_string(),
        }
    }
}
