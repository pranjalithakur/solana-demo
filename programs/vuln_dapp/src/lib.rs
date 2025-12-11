use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};
use solana_program::program::invoke_signed;
use solana_program::system_instruction;

declare_id!("VulnDaPP1111111111111111111111111111111111");

#[program]
pub mod vuln_dapp {
    use super::*;

    /// Simple staking with multiple vulnerabilities:
    /// - missing signer checks on some instructions
    /// - unchecked account owners for token accounts
    /// - time-based lock that trusts client-supplied timestamps
    /// - reinitialization of state
    /// - insecure PDA seeds
    pub fn initialize_pool(ctx: Context<InitializePool>, admin: Pubkey) -> Result<()> {
        let pool = &mut ctx.accounts.pool;
        pool.admin = admin; // VULN: anyone can set arbitrary admin, not restricted to signer
        pool.bump = *ctx.bumps.get("pool").unwrap();
        pool.total_staked = 0;
        pool.lock_seconds = 60; // default
        Ok(())
    }

    pub fn set_lock_seconds(ctx: Context<SetLockSeconds>, new_lock: i64) -> Result<()> {
        // VULN: no admin check; any user can change lock period
        let pool = &mut ctx.accounts.pool;
        pool.lock_seconds = new_lock;
        Ok(())
    }

    pub fn stake(ctx: Context<Stake>, amount: u64, client_now_ts: i64) -> Result<()> {
        // VULN: uses client-provided timestamp instead of on-chain clock
        let pool = &mut ctx.accounts.pool;
        let user_state = &mut ctx.accounts.user_state;

        // VULN: no owner check on user_token_account, could use someone else's account
        let cpi_accounts = Transfer {
            from: ctx.accounts.user_token_account.to_account_info(),
            to: ctx.accounts.pool_vault.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        token::transfer(
            CpiContext::new(cpi_program, cpi_accounts),
            amount,
        )?;

        if user_state.staked_amount == 0 {
            user_state.user = ctx.accounts.user.key();
        }
        user_state.staked_amount = user_state
            .staked_amount
            .checked_add(amount)
            .ok_or(ErrorCode::MathOverflow)?;
        user_state.last_stake_ts = client_now_ts; // VULN
        user_state.bump = *ctx.bumps.get("user_state").unwrap();

        pool.total_staked = pool
            .total_staked
            .checked_add(amount)
            .ok_or(ErrorCode::MathOverflow)?;

        Ok(())
    }

    pub fn unstake(ctx: Context<Unstake>, amount: u64, client_now_ts: i64) -> Result<()> {
        let pool = &mut ctx.accounts.pool;
        let user_state = &mut ctx.accounts.user_state;

        require!(
            user_state.user == ctx.accounts.user.key(),
            ErrorCode::InvalidUser
        );

        // supposed to enforce lock, but uses client_now_ts
        let since_last = client_now_ts
            .checked_sub(user_state.last_stake_ts)
            .ok_or(ErrorCode::MathOverflow)?;
        require!(since_last >= pool.lock_seconds, ErrorCode::StillLocked);

        require!(user_state.staked_amount >= amount, ErrorCode::InsufficientStake);

        user_state.staked_amount = user_state.staked_amount - amount;
        pool.total_staked = pool.total_staked - amount;

        // insecure PDA seeds (no user key, fixed seeds) allow arbitrary withdrawal if derived
        let pool_seeds: &[&[u8]] = &[b"pool", &[pool.bump]];
        let signer_seeds = &[pool_seeds];

        let cpi_accounts = Transfer {
            from: ctx.accounts.pool_vault.to_account_info(),
            to: ctx.accounts.user_token_account.to_account_info(),
            authority: ctx.accounts.pool_signer.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        token::transfer(
            CpiContext::new_with_signer(cpi_program, cpi_accounts, signer_seeds),
            amount,
        )?;

        Ok(())
    }

    pub fn emergency_drain(ctx: Context<EmergencyDrain>) -> Result<()> {
        // Intended only for admin, but missing any admin signature / check at all.
        let pool = &ctx.accounts.pool;

        let pool_seeds: &[&[u8]] = &[b"pool", &[pool.bump]];
        let signer_seeds = &[pool_seeds];

        let balance = ctx.accounts.pool_vault.amount;

        let cpi_accounts = Transfer {
            from: ctx.accounts.pool_vault.to_account_info(),
            to: ctx.accounts.recipient.to_account_info(),
            authority: ctx.accounts.pool_signer.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        token::transfer(
            CpiContext::new_with_signer(cpi_program, cpi_accounts, signer_seeds),
            balance,
        )?;

        Ok(())
    }

    /// Escrow marketplace with additional vulnerabilities:
    /// - trust of arbitrary delegate authority
    /// - lamports withdrawal using unchecked seeds
    /// - reinitialize escrow state
    pub fn create_escrow(
        ctx: Context<CreateEscrow>,
        amount: u64,
        expires_at_client_ts: i64,
    ) -> Result<()> {
        let escrow = &mut ctx.accounts.escrow;
        escrow.maker = ctx.accounts.maker.key();
        escrow.mint = ctx.accounts.mint.key();
        escrow.amount = amount;
        escrow.bump = *ctx.bumps.get("escrow").unwrap();
        escrow.expires_at = expires_at_client_ts; // VULN

        // VULN: no check that token account belongs to maker
        let cpi_accounts = Transfer {
            from: ctx.accounts.maker_token_account.to_account_info(),
            to: ctx.accounts.escrow_vault.to_account_info(),
            authority: ctx.accounts.maker.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        token::transfer(
            CpiContext::new(cpi_program, cpi_accounts),
            amount,
        )?;

        Ok(())
    }

    pub fn cancel_escrow(ctx: Context<CancelEscrow>) -> Result<()> {
        let escrow = &ctx.accounts.escrow;

        // VULN: anyone can cancel; no maker or expiry check

        let escrow_seeds: &[&[u8]] =
            &[b"escrow", escrow.mint.as_ref(), &escrow.bump.to_le_bytes()];
        let signer_seeds = &[escrow_seeds];

        let amount = ctx.accounts.escrow_vault.amount;
        let cpi_accounts = Transfer {
            from: ctx.accounts.escrow_vault.to_account_info(),
            to: ctx.accounts.recipient.to_account_info(), // VULN: arbitrary recipient
            authority: ctx.accounts.escrow_signer.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        token::transfer(
            CpiContext::new_with_signer(cpi_program, cpi_accounts, signer_seeds),
            amount,
        )?;

        Ok(())
    }

    pub fn withdraw_sol_from_treasury(ctx: Context<WithdrawSolFromTreasury>, amount: u64) -> Result<()> {
        let treasury = &ctx.accounts.treasury;

        // VULN: anyone can withdraw SOL as PDA signer, unchecked authority, weak seeds
        let seeds: &[&[u8]] = &[b"treasury", &[treasury.bump]];
        let signer = &[seeds];

        invoke_signed(
            &system_instruction::transfer(
                &ctx.accounts.treasury_signer.key(),
                &ctx.accounts.recipient.key(),
                amount,
            ),
            &[
                ctx.accounts.treasury_signer.to_account_info(),
                ctx.accounts.recipient.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
            signer,
        )?;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializePool<'info> {
    #[account(
        init,
        payer = payer,
        seeds = [b"pool"],
        bump,
        space = 8 + 32 + 8 + 8 + 8
    )]
    pub pool: Account<'info, Pool>,

    /// CHECK: used as PDA signer for vault; seeds are fixed and insecure
    #[account(
        seeds = [b"pool"],
        bump = pool.bump
    )]
    pub pool_signer: UncheckedAccount<'info>,

    #[account(
        init,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = pool_signer
    )]
    pub pool_vault: Account<'info, TokenAccount>,

    pub mint: Account<'info, Mint>,

    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct SetLockSeconds<'info> {
    #[account(mut)]
    pub pool: Account<'info, Pool>,
}

#[derive(Accounts)]
pub struct Stake<'info> {
    #[account(mut)]
    pub pool: Account<'info, Pool>,

    /// CHECK: used as signing PDA; weak seeds
    #[account(
        seeds = [b"pool"],
        bump = pool.bump
    )]
    pub pool_signer: UncheckedAccount<'info>,

    #[account(
        init_if_needed,
        payer = user,
        seeds = [b"user_state", user.key().as_ref()],
        bump,
        space = 8 + 32 + 8 + 8 + 8
    )]
    pub user_state: Account<'info, UserState>,

    #[account(mut)]
    pub pool_vault: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct Unstake<'info> {
    #[account(mut)]
    pub pool: Account<'info, Pool>,

    /// CHECK: used as signing PDA; weak seeds
    #[account(
        seeds = [b"pool"],
        bump = pool.bump
    )]
    pub pool_signer: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [b"user_state", user.key().as_ref()],
        bump = user_state.bump
    )]
    pub user_state: Account<'info, UserState>,

    #[account(mut)]
    pub pool_vault: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct EmergencyDrain<'info> {
    #[account(mut)]
    pub pool: Account<'info, Pool>,

    /// CHECK: used as signing PDA; weak seeds
    #[account(
        seeds = [b"pool"],
        bump = pool.bump
    )]
    pub pool_signer: UncheckedAccount<'info>,

    #[account(mut)]
    pub pool_vault: Account<'info, TokenAccount>,

    #[account(mut)]
    pub recipient: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct CreateEscrow<'info> {
    #[account(mut)]
    pub maker: Signer<'info>,

    pub mint: Account<'info, Mint>,

    #[account(
        init,
        payer = maker,
        seeds = [b"escrow", mint.key().as_ref()],
        bump,
        space = 8 + 32 + 32 + 8 + 8 + 8
    )]
    pub escrow: Account<'info, Escrow>,

    /// CHECK: signer PDA, weak seeds, reused for all escrows of same mint
    #[account(
        seeds = [b"escrow", mint.key().as_ref()],
        bump = escrow.bump
    )]
    pub escrow_signer: UncheckedAccount<'info>,

    #[account(
        init,
        payer = maker,
        associated_token::mint = mint,
        associated_token::authority = escrow_signer
    )]
    pub escrow_vault: Account<'info, TokenAccount>,

    #[account(mut)]
    pub maker_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct CancelEscrow<'info> {
    #[account(mut)]
    pub escrow: Account<'info, Escrow>,

    /// CHECK: signer PDA, weak seeds
    #[account(
        seeds = [b"escrow", escrow.mint.as_ref()],
        bump = escrow.bump
    )]
    pub escrow_signer: UncheckedAccount<'info>,

    #[account(mut)]
    pub escrow_vault: Account<'info, TokenAccount>,

    #[account(mut)]
    pub recipient: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct WithdrawSolFromTreasury<'info> {
    #[account(
        seeds = [b"treasury"],
        bump
    )]
    pub treasury: Account<'info, Treasury>,

    /// CHECK: PDA that actually holds SOL, same weak seeds as treasury
    #[account(
        mut,
        seeds = [b"treasury"],
        bump = treasury.bump
    )]
    pub treasury_signer: UncheckedAccount<'info>,

    #[account(mut)]
    pub recipient: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[account]
pub struct Pool {
    pub admin: Pubkey,
    pub total_staked: u64,
    pub lock_seconds: i64,
    pub bump: u8,
}

#[account]
pub struct UserState {
    pub user: Pubkey,
    pub staked_amount: u64,
    pub last_stake_ts: i64,
    pub bump: u8,
}

#[account]
pub struct Escrow {
    pub maker: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
    pub expires_at: i64,
    pub bump: u8,
}

#[account]
pub struct Treasury {
    pub bump: u8,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Math overflow")]
    MathOverflow,
    #[msg("Invalid user for this position")]
    InvalidUser,
    #[msg("Position still locked")]
    StillLocked,
    #[msg("Insufficient staked amount")]
    InsufficientStake,
}
