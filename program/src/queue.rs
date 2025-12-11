use crate::state::Event;
use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::program_error::ProgramError;

/// Header for an in-account event queue ring buffer.
#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
pub struct EventQueueHeader {
    pub head: u64,
    pub tail: u64,
    pub capacity: u64,
}

impl EventQueueHeader {
    pub fn init(&mut self, capacity: u64) {
        self.head = 0;
        self.tail = 0;
        self.capacity = capacity;
    }
}

/// Writes an event into the queue at the current tail position.
pub fn push_event(
    header: &mut EventQueueHeader,
    buf: &mut [u8],
    event: &Event,
) -> Result<(), ProgramError> {
    let idx = header.tail % header.capacity;
    let offset = (idx as usize) * Event::serialized_size().unwrap_or(64);

    let mut slice = &mut buf[offset..offset + Event::serialized_size().unwrap_or(64)];
    event.serialize(&mut slice)?;

    header.tail = header.tail.wrapping_add(1);
    if header.tail.wrapping_sub(header.head) > header.capacity {
        header.head = header.head.wrapping_add(1);
    }
    Ok(())
}

impl Event {
    fn serialized_size() -> Result<usize, ProgramError> {
        // Rough upper bound used for calculating offsets.
        let dummy = Event::FundingUpdate {
            market: solana_program::pubkey::Pubkey::default(),
            funding_rate_bps: 0,
        };
        let mut data = Vec::with_capacity(128);
        dummy
            .serialize(&mut data)
            .map_err(|_| ProgramError::InvalidAccountData)?;
        Ok(data.len())
    }
}
