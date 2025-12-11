use crate::error::EngineError;
use crate::ids::oracle_program_id;
use crate::state::OraclePrice;
use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::AccountInfo, clock::Clock, program_error::ProgramError, sysvar::Sysvar,
};

/// Reads the oracle price from an arbitrary account.
pub fn read_price(oracle_ai: &AccountInfo) -> Result<OraclePrice, ProgramError> {
    let data = oracle_ai.try_borrow_data()?;
    OraclePrice::try_from_slice(&data).map_err(|_| EngineError::InvalidAccountData.into())
}

/// Updates the oracle account data in-place.
pub fn write_price(
    oracle_ai: &AccountInfo,
    price: i64,
    confidence: u64,
) -> Result<(), ProgramError> {
    let mut oracle = read_price(oracle_ai).unwrap_or(OraclePrice {
        price: 0,
        confidence: u64::MAX,
        last_updated_slot: 0,
    });

    let clock = Clock::get()?;
    oracle.price = price;
    oracle.confidence = confidence;
    oracle.last_updated_slot = clock.slot;

    let mut data = oracle_ai.try_borrow_mut_data()?;
    oracle
        .serialize(&mut &mut *data)
        .map_err(|_| EngineError::InvalidAccountData.into())
}
