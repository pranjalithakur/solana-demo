use crate::error::EngineError;
use solana_program::{
    account_info::AccountInfo, program_error::ProgramError, program_pack::Pack, rent::Rent,
    sysvar::Sysvar,
};

/// Convenience wrapper to assert rent exemption.
pub fn assert_rent_exempt(account: &AccountInfo) -> Result<(), ProgramError> {
    let rent = Rent::get()?;
    if !rent.is_exempt(account.lamports(), account.data_len()) {
        return Err(EngineError::NotRentExempt.into());
    }
    Ok(())
}

/// Returns whether the account data is all zero bytes.
pub fn is_zeroed(account: &AccountInfo) -> bool {
    account.data.borrow().iter().all(|b| *b == 0)
}
