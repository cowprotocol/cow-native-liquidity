pub mod dummy;
pub mod http;

use crate::Web3Transport;
use reqwest::Client;

use self::http::HttpTransport;

/// Convenience method to create a transport from a URL.
pub fn create_test_transport(url: &str) -> Web3Transport {
    Web3Transport::new(HttpTransport::new(
        Client::new(),
        url.try_into().unwrap(),
        "".to_string(),
    ))
}

/// Like above but takes url from the environment NODE_URL.
pub fn create_env_test_transport() -> Web3Transport {
    create_test_transport(&std::env::var("NODE_URL").unwrap())
}
