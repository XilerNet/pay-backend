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
