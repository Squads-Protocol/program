use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    program::invoke_signed,
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
};
use spl_token::state::Account;

use crate::{
    state::squad::Squad,
    *,
};

// (Deprecated)
pub fn process_quit_squad(accounts: &[AccountInfo], program_id: &Pubkey) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let executioner = next_account_info(account_info_iter)?;
    let squad_account = next_account_info(account_info_iter)?;
    let system_program_account = next_account_info(account_info_iter)?;
    let token_program_account = next_account_info(account_info_iter)?;

    if !executioner.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // check the token program
    if *token_program_account.key != spl_token::id() {
        return Err(ProgramError::InvalidAccountData);
    }

    let mut squad_account_info = get_squad(program_id, squad_account)?;

    // check that the member is in the squad
    if !Squad::member_exists(&squad_account_info, executioner.key) {
        return Err(ProgramError::InvalidArgument);
    }

    let mint_owner = next_account_info(account_info_iter)?;
    let member_account = next_account_info(account_info_iter)?;
    let sol_account = next_account_info(account_info_iter)?;

    if sol_account.key != &squad_account_info.sol_account {
        return Err(ProgramError::InvalidAccountData);
    }

    let (mint_owner_address, mint_bump_seed) =
        get_mint_address_with_seed(&squad_account.key, &program_id);
    // check that the gov mint PDA matches the squad one
    if mint_owner_address != *mint_owner.key {
        return Err(ProgramError::InvalidAccountData);
    }

    let mint_signer_seeds: &[&[_]] = &[
        &squad_account.key.to_bytes(),
        b"!squadmint",
        &[mint_bump_seed],
    ];

    let (member_pda, _member_bump_seed) =
        get_equity_address_with_seed(executioner.key, squad_account.key, program_id);

    // check that the member gov PDA matches the account
    if *member_account.key != member_pda {
        return Err(ProgramError::InvalidAccountData);
    }
    // Get account info to know how much to burn
    let member_account_info = Account::unpack_unchecked(&member_account.data.borrow())?;

    // Burn equity token
    invoke_signed(
        &spl_token::instruction::burn(
            &spl_token::id(),
            &member_pda,
            &mint_owner.key,
            &mint_owner.key,
            &[],
            member_account_info.amount,
        )?,
        &[
            member_account.clone(),
            token_program_account.clone(),
            mint_owner.clone(),
        ],
        &[&mint_signer_seeds],
    )?;

    // Close equity account
    invoke_signed(
        &spl_token::instruction::close_account(
            &spl_token::id(),
            &member_account.key,
            &sol_account.key,
            &mint_owner.key,
            &[],
        )?,
        &[
            member_account.clone(),
            sol_account.clone(),
            squad_account.clone(),
            mint_owner.clone(),
            system_program_account.clone(),
        ],
        &[&mint_signer_seeds],
    )?;

    Squad::remove_member(&mut squad_account_info, &executioner.key);
    Squad::pack(squad_account_info, &mut squad_account.data.borrow_mut())?;
    Ok(())
}
