pub mod error;
pub mod instruction;
pub mod processor;
pub mod state;

use solana_program::{
    account_info::AccountInfo,
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack},
    pubkey::Pubkey,
};

use crate::state::{proposal::Proposal, squad::Squad, vote::VoteReceipt};

#[cfg(not(feature = "no-entrypoint"))]
pub mod entrypoint;

pub(crate) type UnixTimestamp = i64;

/// Get the Borsh container count (le) from buffer
pub(crate) fn count_from_le(array: &[u8]) -> usize {
    (array[0] as usize)
        | (array[1] as usize) << 8
        | (array[2] as usize) << 16
        | (array[3] as usize) << 24
}

/// convert a u32 number to byte array of length 4
pub(crate) fn transform_u32_to_array_of_u8(x: u32) -> [u8; 4] {
    let b1: u8 = ((x >> 24) & 0xff) as u8;
    let b2: u8 = ((x >> 16) & 0xff) as u8;
    let b3: u8 = ((x >> 8) & 0xff) as u8;
    let b4: u8 = (x & 0xff) as u8;
    [b4, b3, b2, b1]
}

/// Get the Squad account info after check of ownership
pub(crate) fn get_squad(
    program_id: &Pubkey,
    squad_account: &AccountInfo,
) -> Result<Squad, ProgramError> {
    if squad_account.owner != program_id {
        return Err(ProgramError::IncorrectProgramId);
    }
    let squad_account_info = Squad::unpack_unchecked(&squad_account.data.borrow())?;

    Ok(squad_account_info)
}

/// Get the Proposal account info after check of ownership
pub(crate) fn get_proposal(
    program_id: &Pubkey,
    squad_account: &AccountInfo,
    proposal_account: &AccountInfo,
) -> Result<Proposal, ProgramError> {
    if squad_account.owner != program_id {
        return Err(ProgramError::IncorrectProgramId);
    }
    if proposal_account.owner != program_id {
        return Err(ProgramError::IncorrectProgramId);
    }

    let proposal_account_info = Proposal::unpack_unchecked(&proposal_account.data.borrow())?;

    if proposal_account_info.is_initialized {
        if proposal_account_info.squad_address != *squad_account.key {
            return Err(ProgramError::InvalidAccountData);
        }
    }

    Ok(proposal_account_info)
}

/// Get the Info account info after check of ownership
pub(crate) fn get_vote(
    program_id: &Pubkey,
    squad_account: &AccountInfo,
    vote_account: &AccountInfo,
) -> Result<VoteReceipt, ProgramError> {
    if squad_account.owner != program_id {
        return Err(ProgramError::IncorrectProgramId);
    }
    if vote_account.owner != program_id {
        return Err(ProgramError::IncorrectProgramId);
    }

    let vote_account_info = VoteReceipt::unpack_unchecked(&vote_account.data.borrow())?;

    if vote_account_info.is_initialized {
        return Err(ProgramError::AccountAlreadyInitialized);
    }

    Ok(vote_account_info)
}

/// Get the Squad Mint address from the squad address with the bump seed
pub(crate) fn get_squad_address_with_seed(
    creator_address: &Pubkey,
    random_id: &String,
    program_id: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[&creator_address.to_bytes(), random_id.as_bytes(), b"!squad"],
        &program_id,
    )
}

/// Get the Squad Mint address from the squad address with the bump seed
pub(crate) fn get_mint_address_with_seed(
    squad_address: &Pubkey,
    program_id: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[&squad_address.to_bytes(), b"!squadmint"], &program_id)
}

/// Get the Squad Member Mint address from the squad address with the bump seed
pub(crate) fn get_member_mint_address_with_seed(
    squad_address: &Pubkey,
    program_id: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[&squad_address.to_bytes(), b"!squadmembermint"],
        &program_id,
    )
}

/// Get the Squad sol address from the squad address with the bump seed
pub(crate) fn get_sol_address_with_seed(
    squad_address: &Pubkey,
    program_id: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[&squad_address.to_bytes(), b"!squadsol"], &program_id)
}

/// Get the Squad wsol address from the sol address with the bump seed
pub(crate) fn get_wsol_address_with_seed(
    sol_address: &Pubkey,
    random_id: &String,
    program_id: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[&sol_address.to_bytes(), random_id.as_bytes(), b"!wsol"],
        &program_id,
    )
}

/// Get a users equity account address with bump seed from the users pub key
pub(crate) fn get_equity_address_with_seed(
    member_address: &Pubkey,
    squad_address: &Pubkey,
    program_id: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            &member_address.to_bytes(),
            &squad_address.to_bytes(),
            b"!memberequity",
        ],
        &program_id,
    )
}

/// Get a squad equity account address with bump seed
pub(crate) fn get_squad_equity_address_with_seed(
    squad_address: &Pubkey,
    program_id: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[&squad_address.to_bytes(), b"!squadequity"], &program_id)
}

/// Get a vote source address with bump seed from the member address of the user in question
pub(crate) fn get_source_address_with_seed(
    member_address: &Pubkey,
    program_id: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[&member_address.to_bytes(), br"!source"], &program_id)
}

/// get a vote address with bump seed, for adding a member via vote
pub(crate) fn get_add_member_vote_address_with_seed(
    member_address: &Pubkey,
    program_id: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[&member_address.to_bytes(), br"!addmember"], &program_id)
}

/// Gets the token pda for the squad
/// Supply the squad account address, program_id, and symbol of the token as &String
pub(crate) fn get_token_address_with_seed(
    squad_address: &Pubkey,
    program_id: &Pubkey,
    symbol: &String,
) -> (Pubkey, u8) {
    let mut seedstring = String::from("!squad");
    seedstring.push_str(&symbol.to_ascii_lowercase());
    Pubkey::find_program_address(
        &[&squad_address.to_bytes(), seedstring.as_bytes()],
        &program_id,
    )
}

pub(crate) fn get_proposal_address_with_seed(
    squad_address: &Pubkey,
    program_id: &Pubkey,
    nonce: &u32,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            &squad_address.to_bytes(),
            &nonce.to_le_bytes(),
            b"!proposal",
        ],
        &program_id,
    )
}

pub(crate) fn get_vote_address_with_seed(
    proposal_address: &Pubkey,
    program_id: &Pubkey,
    member_address: &Pubkey,
) -> (Pubkey, u8) {
    let seedstring = String::from("!vote");
    Pubkey::find_program_address(
        &[
            &proposal_address.to_bytes(),
            &member_address.to_bytes(),
            &seedstring.as_bytes(),
        ],
        &program_id,
    )
}
// GET THE SQUAD ADDRESS ONLY
pub fn get_squad_address(
    creator_address: &Pubkey,
    random_id: &String,
    program_id: &Pubkey,
) -> Pubkey {
    get_squad_address_with_seed(&creator_address, &random_id, program_id).0
}

/// Derive the SPL Token mint address associated with a squad account
pub fn get_mint_address(squad_address: &Pubkey, program_id: &Pubkey) -> Pubkey {
    get_mint_address_with_seed(&squad_address, &program_id).0
}

pub fn get_member_mint_address(squad_address: &Pubkey, program_id: &Pubkey) -> Pubkey {
    get_member_mint_address_with_seed(&squad_address, &program_id).0
}

/// Derive the Squad SOL address associated with a squad account
pub fn get_sol_address(squad_address: &Pubkey, program_id: &Pubkey) -> Pubkey {
    get_sol_address_with_seed(&squad_address, &program_id).0
}

/// Derive the Squad WrappedSOL address associated with a sol account
pub fn get_wsol_address(sol_address: &Pubkey, random_id: &String, program_id: &Pubkey) -> Pubkey {
    get_wsol_address_with_seed(&sol_address, random_id, &program_id).0
}

/// Derive the add_member_vote_address associated with a squad account
pub fn get_add_member_vote_address(member_address: &Pubkey, &program_id: &Pubkey) -> Pubkey {
    get_add_member_vote_address_with_seed(&member_address, &program_id).0
}

/// Derive the Member Equity address associated with a squad account
pub fn get_equity_address(
    member_address: &Pubkey,
    squad_address: &Pubkey,
    program_id: &Pubkey,
) -> Pubkey {
    get_equity_address_with_seed(&member_address, &squad_address, &program_id).0
}

/// Derive the Member Equity address associated with a squad account
pub fn get_squad_equity_address(squad_address: &Pubkey, program_id: &Pubkey) -> Pubkey {
    get_squad_equity_address_with_seed(&squad_address, &program_id).0
}

/// Derive the vote source address by the squad member associated with it
pub fn get_source_address(member_address: &Pubkey, program_id: &Pubkey) -> Pubkey {
    get_source_address_with_seed(&member_address, &program_id).0
}

/// Derive the token address by the squad address and symbol
pub fn get_token_address(squad_address: &Pubkey, program_id: &Pubkey, symbol: &String) -> Pubkey {
    get_token_address_with_seed(&squad_address, &program_id, &symbol).0
}

pub fn get_proposal_address(squad_address: &Pubkey, program_id: &Pubkey, nonce: &u32) -> Pubkey {
    get_proposal_address_with_seed(&squad_address, &program_id, &nonce).0
}

pub fn get_vote_address(
    proposal_account: &Pubkey,
    program_id: &Pubkey,
    voter_address: &Pubkey,
) -> Pubkey {
    get_vote_address_with_seed(&proposal_account, &program_id, &voter_address).0
}
