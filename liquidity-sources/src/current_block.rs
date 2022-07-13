use crate::Web3;
use anyhow::{anyhow, Context as _, Result};
use primitive_types::H256;
use std::time::Duration;
use tokio::sync::watch;
use tokio_stream::wrappers::WatchStream;
use web3::{
    types::{BlockId, BlockNumber},
    Transport,
};

pub type Block = web3::types::Block<H256>;

/// Creates a cloneable stream that yields the current block whenever it changes.
///
/// The stream is not guaranteed to yield *every* block individually without gaps but it does yield
/// the newest block whenever it changes. In practice this means that if the node changes the
/// current block in quick succession we might only observe the last block, skipping some blocks in
/// between.
///
/// The stream is cloneable so that we only have to poll the node once while being able to share the
/// result with several consumers. Calling this function again would create a new poller so it is
/// preferable to clone an existing stream instead.
pub async fn current_block_stream(
    web3: Web3,
    poll_interval: Duration,
) -> Result<watch::Receiver<Block>> {
    let first_block = web3.current_block().await?;
    let first_hash = first_block.hash.ok_or_else(|| anyhow!("missing hash"))?;

    let (sender, receiver) = watch::channel(first_block);

    let update_future = async move {
        let mut previous_hash = first_hash;
        loop {
            tokio::time::sleep(poll_interval).await;
            let block = match web3.current_block().await {
                Ok(block) => block,
                Err(err) => {
                    tracing::warn!("failed to get current block: {:?}", err);
                    continue;
                }
            };
            let hash = match block.hash {
                Some(hash) => hash,
                None => {
                    tracing::warn!("missing hash");
                    continue;
                }
            };
            if hash == previous_hash {
                continue;
            }
            if sender.send(block.clone()).is_err() {
                break;
            }
            previous_hash = hash;
        }
    };

    tokio::task::spawn(update_future);
    Ok(receiver)
}

pub type CurrentBlockStream = watch::Receiver<Block>;

pub fn into_stream(receiver: watch::Receiver<Block>) -> WatchStream<Block> {
    WatchStream::new(receiver)
}

pub fn block_number(block: &Block) -> Result<u64> {
    block
        .number
        .map(|number| number.as_u64())
        .ok_or_else(|| anyhow!("no block number"))
}

/// Trait for abstracting the retrieval of the block information such as the
/// latest block number.
#[async_trait::async_trait]
pub trait BlockRetrieving {
    async fn current_block(&self) -> Result<Block>;
    async fn current_block_number(&self) -> Result<u64>;
}

#[async_trait::async_trait]
impl<T> BlockRetrieving for web3::Web3<T>
where
    T: Transport + Send + Sync,
    T::Out: Send,
{
    async fn current_block(&self) -> Result<Block> {
        self.eth()
            .block(BlockId::Number(BlockNumber::Latest))
            .await
            .context("failed to get current block")?
            .ok_or_else(|| anyhow!("no current block"))
    }

    async fn current_block_number(&self) -> Result<u64> {
        Ok(self
            .eth()
            .block_number()
            .await
            .context("failed to get current block number")?
            .as_u64())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::test_transport::TestTransport;
    use ethcontract::dyns::DynTransport;
    use futures::StreamExt;
    use serde_json::json;
    use web3::types::U64;

    #[tokio::test]
    async fn block_stream_test() {
        let mut transport = TestTransport::new();
        let dyn_transport = DynTransport::new(transport.clone());

        for i in 0u64..3u64 {
            let block = Block {
                hash: Some(H256::from_low_u64_be(i)),
                number: Some(U64::from(i)),
                ..Default::default()
            };
            let block = serde_json::to_value(block).unwrap();
            transport.add_response(block);
        }
        let web3 = Web3::new(dyn_transport);
        let receiver = current_block_stream(web3, Duration::from_secs(1))
            .await
            .unwrap();
        let mut stream = into_stream(receiver);
        for _ in 0..3 {
            let block = stream.next().await.unwrap();
            println!("new block number {}", block.number.unwrap().as_u64());
        }

        transport.assert_request("eth_getBlockByNumber", &[json!("latest"), json!(false)]);
        transport.assert_request("eth_getBlockByNumber", &[json!("latest"), json!(false)]);
        transport.assert_request("eth_getBlockByNumber", &[json!("latest"), json!(false)]);
        transport.assert_no_more_requests();
    }
}
