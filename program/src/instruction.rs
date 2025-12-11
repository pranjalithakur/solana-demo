use crate::error::EngineError;
use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::program_error::ProgramError;

/// Instructions supported by the matching engine program.
#[derive(Clone, Debug, BorshSerialize, BorshDeserialize, PartialEq)]
pub enum EngineInstruction {
    InitializeMarket {
        fee_bps: u16,
    },
    Deposit {
        amount: u64,
    },
    Withdraw {
        amount: u64,
    },
    PlaceOrder {
        price_lots: i64,
        max_base_lots: i64,
        side_is_bid: bool,
    },
    CancelOrder {
        order_id: u128,
    },
    UpdateOracle {
        price: i64,
        confidence: u64,
    },
    Liquidate {
        max_liq_amount: u64,
    },
}

impl EngineInstruction {
    /// Unpacks a byte buffer into an [EngineInstruction].
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        Self::try_from_slice(input).map_err(|_| EngineError::InvalidInstruction.into())
    }
}
