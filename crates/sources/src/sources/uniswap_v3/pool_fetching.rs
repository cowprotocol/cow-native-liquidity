use super::graph_api::{RegisteredPools, Token, UniV3SubgraphClient};
use crate::recent_block_cache::Block;
use crate::token_pair::TokenPair;
use anyhow::Result;
use ethcontract::{H160, U256};
use num::BigInt;
use reqwest::Client;
use std::collections::{HashMap, HashSet};

#[async_trait::async_trait]
pub trait PoolFetching: Send + Sync {
    async fn fetch(&self, token_pairs: HashSet<TokenPair>, at_block: Block) -> Result<Vec<Pool>>;
}

pub struct Pool {
    pub address: H160,
    pub token0: Token,
    pub token1: Token,
    pub sqrt_price: U256,
    pub liquidity: U256,
    pub tick: BigInt,
    pub liquidity_net: HashMap<BigInt, BigInt>,
    pub fee_tier: U256,
}

pub struct UniswapV3PoolFetcher {
    pub graph_api: UniV3SubgraphClient,
    pub registered_pools: RegisteredPools,
}

impl UniswapV3PoolFetcher {
    pub async fn new(chain_id: u64, client: Client) -> Result<Self> {
        let graph_api = UniV3SubgraphClient::for_chain(chain_id, client)?;
        Ok(Self {
            registered_pools: graph_api.get_registered_pools().await?,
            graph_api,
        })
    }
}

#[async_trait::async_trait]
impl PoolFetching for UniswapV3PoolFetcher {
    async fn fetch(&self, _token_pairs: HashSet<TokenPair>, _at_block: Block) -> Result<Vec<Pool>> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn uniswap_v3_pool_fetcher_test() {
        let fetcher = UniswapV3PoolFetcher::new(1, Client::new()).await.unwrap();
        assert!(!fetcher.registered_pools.pools.is_empty());
    }
}
