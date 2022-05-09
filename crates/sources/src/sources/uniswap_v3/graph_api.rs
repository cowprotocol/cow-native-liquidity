//! Module containing The Graph API client used for retrieving Uniswap V3
//! pools from the Uniswap V3 subgraph.

use crate::{event_handling::MAX_REORG_BLOCK_COUNT, subgraph::SubgraphClient};
use anyhow::{bail, Result};
use ethcontract::{H160, U256};
use num::BigInt;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;

/// The page size when querying pools.
const QUERY_PAGE_SIZE: usize = 500;

/// A client to the Uniswap V3 subgraph.
///
/// This client is not implemented to allow general GraphQL queries, but instead
/// implements high-level methods that perform GraphQL queries under the hood.
pub struct UniV3SubgraphClient(SubgraphClient);

impl UniV3SubgraphClient {
    /// Creates a new Uniswap V3 subgraph client for the specified chain ID.
    pub fn for_chain(chain_id: u64, client: Client) -> Result<Self> {
        let subgraph_name = match chain_id {
            1 => "uniswap-v3",
            _ => bail!("unsupported chain {}", chain_id),
        };
        Ok(Self(SubgraphClient::new("uniswap", subgraph_name, client)?))
    }

    /// Retrieves the list of registered pools from the subgraph.
    pub async fn get_registered_pools(&self) -> Result<RegisteredPools> {
        use self::pools_query::*;

        let block_number = self.get_safe_block().await?;

        let mut pools = Vec::new();
        let mut last_id = H160::default();

        // We do paging by last ID instead of using `skip`. This is the
        // suggested approach to paging best performance:
        // <https://thegraph.com/docs/graphql-api#pagination>
        loop {
            let page = self
                .0
                .query::<Data>(
                    QUERY,
                    Some(json_map! {
                        "block" => block_number,
                        "pageSize" => QUERY_PAGE_SIZE,
                        "lastId" => json!(last_id),
                    }),
                )
                .await?
                .pools;
            let no_more_pages = page.len() != QUERY_PAGE_SIZE;
            if let Some(last_pool) = page.last() {
                last_id = last_pool.id;
            }

            pools.extend(page);

            if no_more_pages {
                break;
            }
        }

        Ok(RegisteredPools {
            fetched_block_number: block_number,
            pools,
        })
    }

    /// Retrieves a recent block number for which it is safe to assume no
    /// reorgs will happen.
    async fn get_safe_block(&self) -> Result<u64> {
        // Ideally we would want to use block hash here so that we can check
        // that there indeed is no reorg. However, it does not seem possible to
        // retrieve historic block hashes just from the subgraph (it always
        // returns `null`).
        Ok(self
            .0
            .query::<block_number_query::Data>(block_number_query::QUERY, None)
            .await?
            .meta
            .block
            .number
            .saturating_sub(MAX_REORG_BLOCK_COUNT))
    }
}

/// Result of the registered stable pool query.
#[derive(Debug, Default, PartialEq)]
pub struct RegisteredPools {
    /// The block number that the data was fetched
    pub fetched_block_number: u64,
    /// The registered Pools
    pub pools: Vec<PoolData>,
}

/// Pool data from the Uniswap V3 subgraph.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PoolData {
    pub id: H160,
    pub token0: Token,
    pub token1: Token,
    pub fee_tier: U256,
    pub liquidity: U256,
    pub sqrt_price: U256,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub tick: BigInt,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Token {
    pub id: H160,
    pub symbol: String,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub decimals: u8,
}

mod pools_query {
    use super::PoolData;
    use serde::Deserialize;

    pub const QUERY: &str = r#"
        query Pools($block: Int, $pageSize: Int, $lastId: ID) {
            pools(
                block: { number: $block }
                first: $pageSize
                where: {
                    id_gt: $lastId
                    tick_not: null
                }
            ) {
                id
                token0 {
                    symbol
                    id
                    decimals
                }
                token1 {
                    symbol
                    id
                    decimals
                }
                feeTier
                liquidity
                sqrtPrice
                tick
            }
        }
    "#;

    #[derive(Debug, Deserialize, PartialEq)]
    pub struct Data {
        pub pools: Vec<PoolData>,
    }
}

mod block_number_query {
    use serde::Deserialize;

    pub const QUERY: &str = r#"{
        _meta {
            block { number }
        }
    }"#;

    #[derive(Debug, Deserialize, PartialEq)]
    pub struct Data {
        #[serde(rename = "_meta")]
        pub meta: Meta,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    pub struct Meta {
        pub block: Block,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    pub struct Block {
        pub number: u64,
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn decode_pools_data() {
        use pools_query::*;

        assert_eq!(
            serde_json::from_value::<Data>(json!({
                "pools": [
                    {
                      "id": "0x0001fcbba8eb491c3ccfeddc5a5caba1a98c4c28",
                      "token0": {
                        "decimals": "18",
                        "id": "0xbef81556ef066ec840a540595c8d12f516b6378f",
                        "symbol": "BCZ"
                      },
                      "token1": {
                        "decimals": "18",
                        "id": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                        "symbol": "WETH"
                      },
                      "feeTier": "10000",
                      "liquidity": "303015134493562686441",
                      "tick": "-92110",
                      "sqrtPrice": "792216481398733702759960397"
                    },
                    {
                      "id": "0x0002e63328169d7feea121f1e32e4f620abf0352",
                      "token0": {
                        "decimals": "18",
                        "id": "0x0d438f3b5175bebc262bf23753c1e53d03432bde",
                        "symbol": "wNXM"
                      },
                      "token1": {
                        "decimals": "9",
                        "id": "0x903bef1736cddf2a537176cf3c64579c3867a881",
                        "symbol": "ICHI"
                      },
                      "feeTier": "3000",
                      "liquidity": "3125586395511534995",
                      "tick": "-189822",
                      "sqrtPrice": "5986323062404391218190509"
                    }
                ],
            }))
            .unwrap(),
            Data {
                pools: vec![
                    PoolData {
                        id: H160::from_str("0x0001fcbba8eb491c3ccfeddc5a5caba1a98c4c28").unwrap(),
                        token0: Token {
                            id: H160::from_str("0xbef81556ef066ec840a540595c8d12f516b6378f").unwrap(),
                            symbol: "BCZ".to_string(),
                            decimals: 18,
                        },
                        token1: Token {
                            id: H160::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap(),
                            symbol: "WETH".to_string(),
                            decimals: 18,
                        },
                        fee_tier: U256::from_str("10000").unwrap(),
                        liquidity: U256::from_str("303015134493562686441").unwrap(),
                        sqrt_price: U256::from_str("792216481398733702759960397").unwrap(),
                        tick: BigInt::from(-92110),
                    },
                    PoolData {
                        id: H160::from_str("0x0002e63328169d7feea121f1e32e4f620abf0352").unwrap(),
                        token0: Token {
                            id: H160::from_str("0x0d438f3b5175bebc262bf23753c1e53d03432bde").unwrap(),
                            symbol: "wNXM".to_string(),
                            decimals: 18,
                        },
                        token1: Token {
                            id: H160::from_str("0x903bef1736cddf2a537176cf3c64579c3867a881").unwrap(),
                            symbol: "ICHI".to_string(),
                            decimals: 9,
                        },
                        fee_tier: U256::from_str("3000").unwrap(),
                        liquidity: U256::from_str("3125586395511534995").unwrap(),
                        sqrt_price: U256::from_str("5986323062404391218190509").unwrap(),
                        tick: BigInt::from(-189822),
                    },
                ],
            }
        );
    }

    #[test]
    fn decode_block_number_data() {
        use block_number_query::*;

        assert_eq!(
            serde_json::from_value::<Data>(json!({
                "_meta": {
                    "block": {
                        "number": 42,
                    },
                },
            }))
            .unwrap(),
            Data {
                meta: Meta {
                    block: Block { number: 42 }
                }
            }
        );
    }

    #[tokio::test]
    #[ignore]
    async fn uniswap_v3_subgraph_query() {
        let client = UniV3SubgraphClient::for_chain(1, Client::new()).unwrap();
        let result = client.get_registered_pools().await.unwrap();
        println!(
            "Retrieved {} total pools at block {}",
            result.pools.len(),
            result.fetched_block_number,
        );
    }
}
