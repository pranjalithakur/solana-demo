pub mod entrypoint;
pub mod error;
pub mod ids;
pub mod instruction;
pub mod matching;
pub mod oracle;
pub mod processor;
pub mod queue;
pub mod state;
pub mod utils;

use solana_program::pubkey::Pubkey;

solana_program::declare_id!("11111111111111111111111111111111");

/// Returns the program id used by the matching engine.
pub fn program_id() -> Pubkey {
    id()
}
