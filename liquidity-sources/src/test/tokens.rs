//! Mainnet addresses of commonly used tokens.

use ethcontract::H160;

/// Address for the `WETH` token.
pub const WETH: H160 = H160(hex_literal::hex!(
    "c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
));

/// Address for the `GNO` token.
pub const GNO: H160 = H160(hex_literal::hex!(
    "6810e776880c02933d47db1b9fc05908e5386b96"
));

/// Address for the `USDC` token.
pub const USDC: H160 = H160(hex_literal::hex!(
    "A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
));
