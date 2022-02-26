use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program::invoke_signed,
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack},
    pubkey::Pubkey,
    rent::Rent,
    system_instruction::create_account,
    sysvar::Sysvar,
};
use spl_token::instruction::initialize_mint;

use crate::state::squad::AllocationType;
use crate::{state::squad::Squad, *};

pub fn process_create_squad(
    accounts: &[AccountInfo],
    allocation_type: u8,
    vote_support: u8,
    vote_quorum: u8,
    core_threshold: u8,
    squad_name: String,
    description: String,
    token: String,
    random_id: String,
    program_id: &Pubkey,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let payer = next_account_info(account_info_iter)?;

    if !payer.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Check that squad is TC
    if allocation_type != AllocationType::TeamCoordination as u8 {
        return Err(ProgramError::InvalidAccountData);
    }

    let squad_account = next_account_info(account_info_iter)?;
    let mint_owner = next_account_info(account_info_iter)?;
    let token_program_account = next_account_info(account_info_iter)?;
    let system_account = next_account_info(account_info_iter)?;
    let rent_sysvar_info = next_account_info(account_info_iter)?;
    let rent = &Rent::from_account_info(rent_sysvar_info)?;

    let (squad_account_pda, squad_pda_bump) =
        get_squad_address_with_seed(payer.key, &random_id, program_id);
    if squad_account_pda != *squad_account.key {
        msg!("PDAS Do not match");
        return Err(ProgramError::InvalidAccountData);
    }

    if !squad_account.data_is_empty() {
        return Err(ProgramError::AccountAlreadyInitialized);
    }

    // check token program
    if *token_program_account.key != spl_token::id() {
        return Err(ProgramError::InvalidAccountData);
    }

    let squad_signer_seeds: &[&[_]] = &[
        &payer.key.to_bytes(),
        &random_id.as_bytes(),
        b"!squad",
        &[squad_pda_bump],
    ];
    // create the squad account
    invoke_signed(
        &create_account(
            payer.key,
            &squad_account_pda,
            1.max(rent.minimum_balance(Squad::get_packed_len())),
            Squad::get_packed_len() as u64,
            &program_id,
        ),
        &[payer.clone(), squad_account.clone(), system_account.clone()],
        &[&squad_signer_seeds],
    )?;

    let (mint_owner_address, mint_bump_seed) =
        get_mint_address_with_seed(&squad_account.key, &program_id);
    if mint_owner_address != *mint_owner.key {
        msg!("Error: mint address derivation mismatch");
        return Err(ProgramError::InvalidAccountData);
    }
    let mint_signer_seeds: &[&[_]] = &[
        &squad_account.key.to_bytes(),
        b"!squadmint",
        &[mint_bump_seed],
    ];

    // create the squads governance mint account
    invoke_signed(
        &create_account(
            payer.key,
            mint_owner.key,
            1.max(rent.minimum_balance(spl_token::state::Mint::get_packed_len())),
            spl_token::state::Mint::get_packed_len() as u64,
            &spl_token::id(),
        ),
        &[payer.clone(), mint_owner.clone(), system_account.clone()],
        &[&mint_signer_seeds],
    )?;

    // initialize the squad governance mint account
    invoke_signed(
        &initialize_mint(&spl_token::id(), mint_owner.key, mint_owner.key, None, 0)?,
        &[
            token_program_account.clone(),
            rent_sysvar_info.clone(),
            mint_owner.clone(),
        ],
        &[&mint_signer_seeds],
    )?;

    let mut squad_info = get_squad(program_id, squad_account)?;

    if squad_info.is_initialized() {
        return Err(ProgramError::AccountAlreadyInitialized);
    }

    // check support and quorum are within range
    if vote_quorum < 1 || vote_quorum > 100 {
        return Err(ProgramError::InvalidArgument);
    }

    if vote_support < 1 || vote_support > 100 {
        return Err(ProgramError::InvalidArgument);
    }

    let (sol_account_owner_pda, _sol_account_bump_seed) =
        get_sol_address_with_seed(&squad_account.key, &program_id);

    Squad::setup_tc(
        &mut squad_info,
        1,
        vote_support,
        vote_quorum,
        core_threshold,
        squad_name,
        description,
        token,
        payer.key,
        mint_owner.key,
        &sol_account_owner_pda,
        random_id,
    );

    Squad::pack(squad_info, &mut squad_account.data.borrow_mut())?;
    Ok(())
}
