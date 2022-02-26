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

use crate::{
    state::squad::{Member, Squad},
    *,
};

pub fn process_create_multisig(
    accounts: &[AccountInfo],
    vote_quorum: u8,
    squad_name: String,
    description: String,
    random_id: String,
    members_num: u8,
    program_id: &Pubkey,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let payer = next_account_info(account_info_iter)?;

    if !payer.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    let squad_account = next_account_info(account_info_iter)?;
    let system_account = next_account_info(account_info_iter)?;
    let rent_sysvar_info = next_account_info(account_info_iter)?;
    next_account_info(account_info_iter)?; // skip program account

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

    let mut squad_info = get_squad(program_id, squad_account)?;

    if squad_info.is_initialized() {
        return Err(ProgramError::AccountAlreadyInitialized);
    }

    // check that quorum is within bounds
    // multisig is capped at max owners
    if vote_quorum < 1 || vote_quorum > members_num {
        return Err(ProgramError::InvalidArgument);
    }

    let (sol_account_owner_pda, _sol_account_bump_seed) =
        get_sol_address_with_seed(&squad_account.key, &program_id);

    for _i in 0..members_num {
        let member = next_account_info(account_info_iter)?;

        squad_info.members.insert(
            *member.key,
            Member {
                equity_token_account: *member.key,
            },
        );
    }

    Squad::setup_ms(
        &mut squad_info,
        vote_quorum,
        squad_name,
        description,
        payer.key,
        &sol_account_owner_pda,
        random_id,
    );

    Squad::pack(squad_info, &mut squad_account.data.borrow_mut())?;
    Ok(())
}
