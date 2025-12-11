use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{clock::UnixTimestamp, pubkey::Pubkey};

/// Configuration for a single trading market.
#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
pub struct Market {
    pub admin: Pubkey,
    pub base_mint: Pubkey,
    pub quote_mint: Pubkey,
    pub oracle: Pubkey,
    pub fee_bps: u16,
    pub is_active: bool,
    pub padding: [u8; 5],
}

/// User account tracking balances and open orders.
#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
pub struct UserAccount {
    pub owner: Pubkey,
    pub market: Pubkey,
    pub base_position: i64,
    pub quote_position: i64,
    pub last_update_ts: UnixTimestamp,
    pub open_orders: [Order; 8],
}

/// Compact in-memory representation of a single order.
#[derive(Copy, Clone, Debug, Default, BorshSerialize, BorshDeserialize)]
pub struct Order {
    pub id: u128,
    pub price_lots: i64,
    pub base_lots: i64,
    pub side_is_bid: bool,
    pub is_active: bool,
}

/// Oracle price record stored on-chain.
#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
pub struct OraclePrice {
    pub price: i64,
    pub confidence: u64,
    pub last_updated_slot: u64,
}

/// Event types pushed into a ring buffer queue.
#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
pub enum Event {
    Trade {
        maker: Pubkey,
        taker: Pubkey,
        price_lots: i64,
        base_lots: i64,
    },
    FundingUpdate {
        market: Pubkey,
        funding_rate_bps: i64,
    },
}
