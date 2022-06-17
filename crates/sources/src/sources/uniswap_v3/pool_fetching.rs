use super::graph_api::{PoolData, UniV3SubgraphClient};
use crate::token_pair::TokenPair;
use anyhow::{Context, Result};
use ethcontract::H160;
use itertools::{Either, Itertools};
use reqwest::Client;
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, Weak},
    time::{Duration, Instant},
};

#[async_trait::async_trait]
pub trait PoolFetching: Send + Sync {
    async fn fetch(&self, token_pairs: &HashSet<TokenPair>) -> Result<Vec<PoolData>>;
}

pub struct CachedPool {
    pub pool: PoolData,
    pub updated_at: Instant,
    pub requested_at: Instant,
}

pub struct UniswapV3PoolFetcher {
    graph_api: UniV3SubgraphClient,
    /// H160 is pool id while TokenPair is a pair or tokens for each pool
    pools_by_token_pair: HashMap<TokenPair, HashSet<H160>>,
    cache: Mutex<HashMap<H160, CachedPool>>,
    max_age: Duration,
}

impl UniswapV3PoolFetcher {
    /// Retrieves all registered pools on Uniswap V3 subgraph, but without `ticks`,
    /// making the cache values outdated immediately. Cache values are supposed to be updated
    /// either on fetch or on periodic maintenance update.
    pub async fn new(chain_id: u64, max_age: Duration, client: Client) -> Result<Self> {
        let graph_api = UniV3SubgraphClient::for_chain(chain_id, client)?;
        let registered_pools = graph_api.get_registered_pools().await?;
        tracing::debug!(
            block = %registered_pools.fetched_block_number, pools = %registered_pools.pools.len(),
            "initialized registered pools",
        );

        let mut pools_by_token_pair: HashMap<TokenPair, HashSet<H160>> = HashMap::new();
        for pool in registered_pools.pools {
            let token0 = pool.token0.clone().context("token0 does not exist")?.id;
            let token1 = pool.token1.clone().context("token1 does not exist")?.id;

            let pair = TokenPair::new(token0, token1).context("cant create pair")?;
            pools_by_token_pair.entry(pair).or_default().insert(pool.id);
        }

        Ok(Self {
            pools_by_token_pair,
            graph_api,
            cache: Default::default(),
            max_age,
        })
    }

    async fn get_pools_and_update_cache(&self, pool_ids: &[H160]) -> Result<Vec<PoolData>> {
        let pools = self.graph_api.get_pools_with_ticks_by_ids(pool_ids).await?;
        let now = Instant::now();
        let mut cache = self.cache.lock().unwrap();
        for pool in &pools {
            cache.insert(
                pool.id,
                CachedPool {
                    pool: pool.clone(),
                    updated_at: now,
                    requested_at: now,
                },
            );
        }
        Ok(pools)
    }

    /// Returns cached pools and ids of outdated pools.
    fn get_cached_pools(&self, token_pairs: &HashSet<TokenPair>) -> (Vec<PoolData>, Vec<H160>) {
        let mut pool_ids = token_pairs
            .iter()
            .filter_map(|pair| self.pools_by_token_pair.get(pair))
            .flatten()
            .peekable();

        match pool_ids.peek() {
            Some(_) => {
                let now = Instant::now();
                let mut cache = self.cache.lock().unwrap();
                pool_ids.partition_map(|pool_id| match cache.get_mut(pool_id) {
                    Some(entry)
                        if now.saturating_duration_since(entry.updated_at) < self.max_age =>
                    {
                        entry.requested_at = now;
                        Either::Left(entry.pool.clone())
                    }
                    _ => Either::Right(pool_id),
                })
            }
            None => Default::default(),
        }
    }
}

#[async_trait::async_trait]
impl PoolFetching for UniswapV3PoolFetcher {
    async fn fetch(&self, token_pairs: &HashSet<TokenPair>) -> Result<Vec<PoolData>> {
        let (mut cached_pools, outdated_pools) = self.get_cached_pools(token_pairs);

        if !outdated_pools.is_empty() {
            let updated_pools = self.get_pools_and_update_cache(&outdated_pools).await?;
            cached_pools.extend(updated_pools);
        }

        Ok(cached_pools)
    }
}

pub struct AutoUpdatingUniswapV3PoolFetcher(Arc<UniswapV3PoolFetcher>);

impl AutoUpdatingUniswapV3PoolFetcher {
    /// Creates new CachingUniswapV3PoolFetcher with the purpose of spawning an additional
    /// background task for periodic update of cache
    pub async fn new(chain_id: u64, max_age: Duration, client: Client) -> Result<Self> {
        Ok(Self(Arc::new(
            UniswapV3PoolFetcher::new(chain_id, max_age, client).await?,
        )))
    }

    /// Spawns a background task maintaining the cache once per `update_interval`.
    /// Only soon to be outdated pools get updated and recently used pools have a higher priority.
    /// If `update_size` is `Some(n)` at most `n` pools get updated per interval.
    /// If `update_size` is `None` no limit gets applied.
    pub fn spawn_maintenance_task(&self, update_interval: Duration, update_size: Option<usize>) {
        tokio::spawn(update_recently_used_outdated_pools(
            Arc::downgrade(&self.0),
            update_interval,
            update_size,
        ));
    }
}

#[async_trait::async_trait]
impl PoolFetching for AutoUpdatingUniswapV3PoolFetcher {
    async fn fetch(&self, token_pairs: &HashSet<TokenPair>) -> Result<Vec<PoolData>> {
        self.0.fetch(token_pairs).await
    }
}

async fn update_recently_used_outdated_pools(
    inner: Weak<UniswapV3PoolFetcher>,
    update_interval: Duration,
    update_size: Option<usize>,
) {
    while let Some(inner) = inner.upgrade() {
        let now = Instant::now();

        let mut outdated_entries = inner
            .cache
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, cached)| now.saturating_duration_since(cached.updated_at) > inner.max_age)
            .map(|(pool_id, cached)| (*pool_id, cached.requested_at))
            .collect::<Vec<_>>();
        outdated_entries.sort_by_key(|entry| std::cmp::Reverse(entry.1));

        let pools_to_update = outdated_entries
            .iter()
            .take(update_size.unwrap_or(outdated_entries.len()))
            .map(|(pool_id, _)| *pool_id)
            .collect::<Vec<_>>();

        if !pools_to_update.is_empty() {
            if let Err(err) = inner.get_pools_and_update_cache(&pools_to_update).await {
                tracing::warn!(
                    error = %err,
                    "failed to update pools",
                );
            }
        }

        tokio::time::sleep(update_interval.saturating_sub(now.elapsed())).await;
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[tokio::test]
    #[ignore]
    async fn uniswap_v3_pool_fetcher_test() {
        let fetcher = UniswapV3PoolFetcher::new(1, Duration::from_secs(10), Client::new())
            .await
            .unwrap();

        assert!(!fetcher.pools_by_token_pair.is_empty());
        assert!(!fetcher.cache.lock().unwrap().is_empty());
    }

    #[tokio::test]
    #[ignore]
    async fn caching_uniswap_v3_pool_fetcher_test() {
        let fetcher = AutoUpdatingUniswapV3PoolFetcher::new(1, Duration::from_secs(10), Client::new())
            .await
            .unwrap();

        fetcher.spawn_maintenance_task(Duration::from_secs(1), Some(50));

        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    #[tokio::test]
    #[ignore]
    async fn fetch_test() {
        let fetcher = AutoUpdatingUniswapV3PoolFetcher::new(1, Duration::from_secs(10), Client::new())
            .await
            .unwrap();
        let token_pairs = HashSet::from([TokenPair::new(
            H160::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").unwrap(),
            H160::from_str("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48").unwrap(),
        )
        .unwrap()]);
        let pools = fetcher.fetch(&token_pairs).await.unwrap();
        assert!(!pools.is_empty());
    }
}
