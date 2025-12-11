use solana_program::program_error::ProgramError;
use thiserror::Error;

/// Errors that may be returned by the matching engine program.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum EngineError {
    #[error("Invalid instruction data")]
    InvalidInstruction,
    #[error("Account is not rent exempt")]
    NotRentExempt,
    #[error("Account has unexpected owner")]
    InvalidOwner,
    #[error("Account has unexpected data layout")]
    InvalidAccountData,
    #[error("Math operation overflowed or underflowed")]
    MathError,
    #[error("Market is inactive")]
    MarketInactive,
    #[error("Unauthorized operation")]
    Unauthorized,
}

impl From<EngineError> for ProgramError {
    fn from(e: EngineError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
