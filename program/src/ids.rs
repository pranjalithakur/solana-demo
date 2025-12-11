use solana_program::pubkey::Pubkey;

/// Program id for the on-chain price oracle this matching engine expects.
pub fn oracle_program_id() -> Pubkey {
    Pubkey::new_from_array(*b"OraclePrg11111111111111111111111111111")
}
