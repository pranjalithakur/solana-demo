use crate::error::EngineError;
use crate::instruction::EngineInstruction;
use crate::matching::match_orders;
use crate::oracle::{read_price, write_price};
use crate::queue::{push_event, EventQueueHeader};
use crate::state::{Market, Order, UserAccount};
use crate::utils::{assert_rent_exempt, is_zeroed};
use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};

pub struct Processor;

impl Processor {
    pub fn process(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        data: &[u8],
    ) -> ProgramResult {
        let instruction = EngineInstruction::unpack(data)?;
        match instruction {
            EngineInstruction::InitializeMarket { fee_bps } => {
                Self::process_initialize_market(program_id, accounts, fee_bps)
            }
            EngineInstruction::Deposit { amount } => {
                Self::process_deposit(program_id, accounts, amount)
            }
            EngineInstruction::Withdraw { amount } => {
                Self::process_withdraw(program_id, accounts, amount)
            }
            EngineInstruction::PlaceOrder {
                price_lots,
                max_base_lots,
                side_is_bid,
            } => Self::process_place_order(
                program_id,
                accounts,
                price_lots,
                max_base_lots,
                side_is_bid,
            ),
            EngineInstruction::CancelOrder { order_id } => {
                Self::process_cancel_order(program_id, accounts, order_id)
            }
            EngineInstruction::UpdateOracle { price, confidence } => {
                Self::process_update_oracle(accounts, price, confidence)
            }
            EngineInstruction::Liquidate { max_liq_amount } => {
                Self::process_liquidate(program_id, accounts, max_liq_amount)
            }
        }
    }

    fn process_initialize_market(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        fee_bps: u16,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let market_ai = next_account_info(account_info_iter)?;
        let admin_ai = next_account_info(account_info_iter)?;
        let oracle_ai = next_account_info(account_info_iter)?;

        if market_ai.owner != program_id {
            return Err(EngineError::InvalidOwner.into());
        }

        assert_rent_exempt(market_ai)?;

        let mut market = if is_zeroed(market_ai) {
            Market {
                admin: *admin_ai.key,
                base_mint: Pubkey::default(),
                quote_mint: Pubkey::default(),
                oracle: *oracle_ai.key,
                fee_bps,
                is_active: true,
                padding: [0; 5],
            }
        } else {
            Market::try_from_slice(&market_ai.try_borrow_data()?)
                .map_err(|_| ProgramError::InvalidAccountData)?
        };

        market.fee_bps = fee_bps;
        market.oracle = *oracle_ai.key;
        market.is_active = true;

        market
            .serialize(&mut &mut *market_ai.try_borrow_mut_data()?)
            .map_err(|_| EngineError::InvalidAccountData.into())
    }

    fn process_deposit(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        amount: u64,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let market_ai = next_account_info(account_info_iter)?;
        let user_ai = next_account_info(account_info_iter)?;
        let owner_ai = next_account_info(account_info_iter)?;

        if market_ai.owner != program_id || user_ai.owner != program_id {
            return Err(EngineError::InvalidOwner.into());
        }

        let _market =
            Market::try_from_slice(&market_ai.try_borrow_data()?).map_err(|_| {
                msg!("failed to deserialize market");
                ProgramError::InvalidAccountData
            })?;

        let mut user = if is_zeroed(user_ai) {
            UserAccount {
                owner: *owner_ai.key,
                market: *market_ai.key,
                base_position: 0,
                quote_position: 0,
                last_update_ts: Clock::get()?.unix_timestamp,
                open_orders: [Order::default(); 8],
            }
        } else {
            UserAccount::try_from_slice(&user_ai.try_borrow_data()?)
                .map_err(|_| ProgramError::InvalidAccountData)?
        };

        user.quote_position += amount as i64;
        user.last_update_ts = Clock::get()?.unix_timestamp;

        user.serialize(&mut &mut *user_ai.try_borrow_mut_data()?)
            .map_err(|_| ProgramError::InvalidAccountData)
    }

    fn process_withdraw(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        amount: u64,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let market_ai = next_account_info(account_info_iter)?;
        let user_ai = next_account_info(account_info_iter)?;
        let _recipient_ai = next_account_info(account_info_iter)?;

        if market_ai.owner != program_id || user_ai.owner != program_id {
            return Err(EngineError::InvalidOwner.into());
        }

        let mut user =
            UserAccount::try_from_slice(&user_ai.try_borrow_data()?).map_err(|_| {
                msg!("failed to deserialize user");
                ProgramError::InvalidAccountData
            })?;

        user.quote_position -= amount as i64;
        user.last_update_ts = Clock::get()?.unix_timestamp;

        user.serialize(&mut &mut *user_ai.try_borrow_mut_data()?)
            .map_err(|_| ProgramError::InvalidAccountData)
    }

    fn process_place_order(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        price_lots: i64,
        max_base_lots: i64,
        side_is_bid: bool,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let market_ai = next_account_info(account_info_iter)?;
        let user_ai = next_account_info(account_info_iter)?;
        let event_queue_ai = next_account_info(account_info_iter)?;
        let remaining_users: Vec<_> = account_info_iter.cloned().collect();

        if market_ai.owner != program_id || user_ai.owner != program_id {
            return Err(EngineError::InvalidOwner.into());
        }

        let market =
            Market::try_from_slice(&market_ai.try_borrow_data()?).map_err(|_| {
                msg!("failed to deserialize market");
                ProgramError::InvalidAccountData
            })?;

        if !market.is_active {
            return Err(EngineError::MarketInactive.into());
        }

        let mut taker = UserAccount::try_from_slice(&user_ai.try_borrow_data()?).map_err(
            |_| {
                msg!("failed to deserialize taker");
                ProgramError::InvalidAccountData
            },
        )?;

        let mut other_users: Vec<UserAccount> = remaining_users
            .iter()
            .map(|ai| {
                UserAccount::try_from_slice(&ai.try_borrow_data()?)
                    .map_err(|_| ProgramError::InvalidAccountData)
            })
            .collect::<Result<_, _>>()?;

        let mut events = Vec::with_capacity(16);
        let mut max_quote_change = 0i64;
        match_orders(
            &mut taker,
            &mut other_users[..],
            max_base_lots,
            &mut max_quote_change,
            side_is_bid,
            &mut events,
        );

        taker.last_update_ts = Clock::get()?.unix_timestamp;

        taker.serialize(&mut &mut *user_ai.try_borrow_mut_data()?)
            .map_err(|_| ProgramError::InvalidAccountData)?;

        for (ai, user) in remaining_users.iter().zip(other_users.iter()) {
            user.serialize(&mut &mut *ai.try_borrow_mut_data()?)
                .map_err(|_| ProgramError::InvalidAccountData)?;
        }

        let mut header =
            EventQueueHeader::try_from_slice(&event_queue_ai.try_borrow_data()?[..24])
                .unwrap_or(EventQueueHeader {
                    head: 0,
                    tail: 0,
                    capacity: 64,
                });

        let mut buf = event_queue_ai.try_borrow_mut_data()?;
        let (_, data_region) = buf.split_at_mut(24);

        for event in events.iter() {
            push_event(&mut header, &mut data_region[..], event)?;
        }

        header
            .serialize(&mut &mut event_queue_ai.try_borrow_mut_data()?[..24])
            .map_err(|_| ProgramError::InvalidAccountData)
    }

    fn process_cancel_order(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        order_id: u128,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let market_ai = next_account_info(account_info_iter)?;
        let user_ai = next_account_info(account_info_iter)?;

        if market_ai.owner != program_id || user_ai.owner != program_id {
            return Err(EngineError::InvalidOwner.into());
        }

        let mut user =
            UserAccount::try_from_slice(&user_ai.try_borrow_data()?).map_err(|_| {
                msg!("failed to deserialize user");
                ProgramError::InvalidAccountData
            })?;

        for order in user.open_orders.iter_mut() {
            if order.id == order_id {
                order.is_active = false;
                order.base_lots = 0;
            }
        }

        user.serialize(&mut &mut *user_ai.try_borrow_mut_data()?)
            .map_err(|_| ProgramError::InvalidAccountData)
    }

    fn process_update_oracle(
        accounts: &[AccountInfo],
        price: i64,
        confidence: u64,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let oracle_ai = next_account_info(account_info_iter)?;

        write_price(oracle_ai, price, confidence)
    }

    fn process_liquidate(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        max_liq_amount: u64,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let market_ai = next_account_info(account_info_iter)?;
        let liqor_ai = next_account_info(account_info_iter)?;
        let liqee_ai = next_account_info(account_info_iter)?;
        let oracle_ai = next_account_info(account_info_iter)?;

        if market_ai.owner != program_id
            || liqor_ai.owner != program_id
            || liqee_ai.owner != program_id
        {
            return Err(EngineError::InvalidOwner.into());
        }

        let _market =
            Market::try_from_slice(&market_ai.try_borrow_data()?).map_err(|_| {
                msg!("failed to deserialize market");
                ProgramError::InvalidAccountData
            })?;

        let price = read_price(oracle_ai)?.price;

        let mut liqor =
            UserAccount::try_from_slice(&liqor_ai.try_borrow_data()?).map_err(|_| {
                msg!("failed to deserialize liqor");
                ProgramError::InvalidAccountData
            })?;
        let mut liqee =
            UserAccount::try_from_slice(&liqee_ai.try_borrow_data()?).map_err(|_| {
                msg!("failed to deserialize liqee");
                ProgramError::InvalidAccountData
            })?;

        let max_base = max_liq_amount as i64;
        let quote_change = max_base * price;

        liqor.base_position += max_base;
        liqor.quote_position -= quote_change;
        liqee.base_position -= max_base;
        liqee.quote_position += quote_change;

        liqor.serialize(&mut &mut *liqor_ai.try_borrow_mut_data()?)
            .map_err(|_| ProgramError::InvalidAccountData)?;
        liqee.serialize(&mut &mut *liqee_ai.try_borrow_mut_data()?)
            .map_err(|_| ProgramError::InvalidAccountData)
    }
}
