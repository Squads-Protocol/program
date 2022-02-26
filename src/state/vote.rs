use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    program_error::ProgramError,
    program_pack::{Pack, Sealed},
    pubkey::Pubkey,
};

use crate::UnixTimestamp;

const VOTE_INITIALIZED_BYTES: usize = 1;
const PUBLIC_KEY_BYTES: usize = 32;
const VOTE_CAST_BYTES: usize = 1;
const TIMESTAMP_BYTES: usize = 8;
const WEIGHT_BYTES: usize = 8;
const VOTE_RECORD_RESERVED_BYTES: usize = 8 * 4;

const VOTE_RECEIPT_TOTAL_BYTES: usize = VOTE_INITIALIZED_BYTES + // is_initialized 1
    PUBLIC_KEY_BYTES +                      // proposal address 32
    VOTE_CAST_BYTES +                // vote cast 1
    PUBLIC_KEY_BYTES +                      // voter address 32
    TIMESTAMP_BYTES +                       // description of the proposal 8
    WEIGHT_BYTES +                       // weight of the voter 8
    VOTE_RECORD_RESERVED_BYTES; // reserved for updates

// State of vote that has been cast (proof)
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
pub struct VoteReceipt {
    pub is_initialized: bool,
    pub proposal_address: Pubkey,
    // can only be 1 of 5 total options
    pub vote_cast: u8,
    pub voter: Pubkey,
    pub cast_timestamp: UnixTimestamp,
    pub weight: u64,

    // reserved for future updates
    pub reserved: [u64; 4],
}

impl Sealed for VoteReceipt {}

impl VoteReceipt {
    pub fn save_vote(
        &mut self,
        proposal_account: &Pubkey,
        vote: u8,
        voter: &Pubkey,
        cast_timestamp: i64,
        weight: u64,
    ) {
        self.is_initialized = true;
        self.proposal_address = *proposal_account;
        self.vote_cast = vote;
        self.voter = *voter;
        self.cast_timestamp = cast_timestamp;
        self.weight = weight;
    }
}

impl Pack for VoteReceipt {
    const LEN: usize = VOTE_RECEIPT_TOTAL_BYTES;
    fn pack_into_slice(&self, dst: &mut [u8]) {
        let dst = array_mut_ref![dst, 0, VOTE_RECEIPT_TOTAL_BYTES];

        let (
            is_initialized_dst,
            proposal_address_dst,
            // can only be 1 of 16 total options
            vote_cast_dst,
            voter_dst,
            cast_timestamp_dst,
            weight_dst,
            _reserved,
        ) = mut_array_refs![
            dst,
            VOTE_INITIALIZED_BYTES, // is_initialized 1
            PUBLIC_KEY_BYTES,       // proposal address 32
            VOTE_CAST_BYTES,        // vote cast 1
            PUBLIC_KEY_BYTES,       // voter address 32
            TIMESTAMP_BYTES,        // description of the proposal 8
            WEIGHT_BYTES,           // weight of the voter 8
            VOTE_RECORD_RESERVED_BYTES
        ];

        let VoteReceipt {
            is_initialized,
            proposal_address,
            // can only be 1 of 5 total options
            vote_cast,
            voter,
            cast_timestamp,
            weight,
            reserved: _,
        } = self;

        is_initialized_dst[0] = *is_initialized as u8;
        *proposal_address_dst = proposal_address.to_bytes();
        vote_cast_dst[0] = *vote_cast;
        *voter_dst = voter.to_bytes();
        *cast_timestamp_dst = cast_timestamp.to_le_bytes();
        *weight_dst = weight.to_le_bytes();
    }

    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let src = array_ref![src, 0, VOTE_RECEIPT_TOTAL_BYTES];
        let (
            is_initialized,
            proposal_address_src,
            // can only be 1 of 5 total options
            vote_cast_src,
            voter_src,
            cast_timestamp_src,
            weight_src,
            _reserved,
        ) = array_refs![
            src,
            VOTE_INITIALIZED_BYTES, // is_initialized
            PUBLIC_KEY_BYTES,       // proposal address
            VOTE_CAST_BYTES,        // vote cast
            PUBLIC_KEY_BYTES,       // voter
            TIMESTAMP_BYTES,
            WEIGHT_BYTES,
            VOTE_RECORD_RESERVED_BYTES
        ];

        let is_initialized = match is_initialized {
            [0] => false,
            [1] => true,
            _ => return Err(ProgramError::InvalidAccountData),
        };

        Ok(VoteReceipt {
            is_initialized,
            proposal_address: Pubkey::new(proposal_address_src),
            vote_cast: vote_cast_src[0],
            voter: Pubkey::new(voter_src),
            cast_timestamp: i64::from_le_bytes(*cast_timestamp_src),
            weight: u64::from_le_bytes(*weight_src),
            reserved: [0; 4],
        })
    }
}

#[cfg(test)]
mod tests {}
