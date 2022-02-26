use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program::invoke,
    program::invoke_signed,
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
    rent::Rent,
    system_instruction::create_account,
    system_instruction::{allocate, assign, transfer},
    sysvar::Sysvar,
};
use spl_token::instruction::initialize_account;

use crate::{
    state::squad::{Member, Squad},
    *,
};

pub fn process_add_members_to_squad(
    accounts: &[AccountInfo],
    members_num: u8,
    allocation_table: Vec<u64>,
    program_id: &Pubkey,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let initializer = next_account_info(account_info_iter)?;
    let squad_account = next_account_info(account_info_iter)?;

    let mut squad_info = get_squad(program_id, squad_account)?;

    if !initializer.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    if !squad_info.open {
        return Err(ProgramError::InvalidAccountData);
    }
    if *initializer.key != squad_info.admin {
        return Err(ProgramError::InvalidAccountData);
    }

    let mint_owner = next_account_info(account_info_iter)?;
    let token_program_account = next_account_info(account_info_iter)?;
    let system_account = next_account_info(account_info_iter)?;
    let rent_sysvar_info = next_account_info(account_info_iter)?;
    let rent = &Rent::from_account_info(rent_sysvar_info)?;

    //
    // Squad mint creation
    //
    // create the account for the squad mint
    // first derive the PDA of the mint
    let (mint_owner_address, mint_bump_seed) =
        get_mint_address_with_seed(&squad_account.key, &program_id);

    // check that the mint matches the account provided
    if mint_owner_address != *mint_owner.key {
        return Err(ProgramError::InvalidArgument);
    }
    let mint_signer_seeds: &[&[_]] = &[
        &squad_account.key.to_bytes(),
        b"!squadmint",
        &[mint_bump_seed],
    ];

    // check that the squad PDA matches the one provided
    let (squad_account_pda, _squad_pda_bump) =
        get_squad_address_with_seed(initializer.key, &squad_info.random_id, program_id);
    if squad_account_pda != *squad_account.key {
        msg!("PDAS Do not match");
        return Err(ProgramError::InvalidArgument);
    }

    for member_index in 0..members_num {
        // member wallet address
        let member = next_account_info(account_info_iter)?;
        // member governance PDA
        let member_account = next_account_info(account_info_iter)?;

        squad_info.members.insert(
            *member.key,
            Member {
                equity_token_account: *member_account.key,
            },
        );

        // derive the PDA from the member and squad keys
        let (member_pda, member_bump_seed) =
            get_equity_address_with_seed(member.key, squad_account.key, program_id);
        let member_signer_seeds: &[&[_]] = &[
            &member.key.to_bytes(),
            &squad_account.key.to_bytes(),
            b"!memberequity",
            &[member_bump_seed],
        ];
        // check that the members governance PDA matches the one provided
        if *member_account.key != member_pda {
            return Err(ProgramError::InvalidAccountData);
        }

        // DoS check
        let rent_exempt_lamports = rent
            .minimum_balance(spl_token::state::Account::get_packed_len())
            .max(1);
        if member_account.lamports() > 0 {
            let top_up_lamports = rent_exempt_lamports.saturating_sub(member_account.lamports());

            if top_up_lamports > 0 {
                invoke(
                    &transfer(initializer.key, member_account.key, top_up_lamports),
                    &[
                        initializer.clone(),
                        member_account.clone(),
                        system_account.clone(),
                    ],
                )?;
            }

            invoke_signed(
                &allocate(
                    member_account.key,
                    spl_token::state::Account::get_packed_len() as u64,
                ),
                &[member_account.clone(), system_account.clone()],
                &[&member_signer_seeds],
            )?;

            invoke_signed(
                &assign(member_account.key, &spl_token::id()),
                &[member_account.clone(), system_account.clone()],
                &[&member_signer_seeds],
            )?;
        } else {
            // create the equity token account for the member
            invoke_signed(
                &create_account(
                    initializer.key,
                    &member_pda,
                    1.max(rent.minimum_balance(spl_token::state::Account::get_packed_len())),
                    spl_token::state::Account::get_packed_len() as u64,
                    &spl_token::id(),
                ),
                &[
                    initializer.clone(),
                    member_account.clone(),
                    system_account.clone(),
                ],
                &[&member_signer_seeds],
            )?;
        }
        // initialize the equity token account for the member
        invoke_signed(
            &initialize_account(
                &spl_token::id(),
                &member_pda,
                &mint_owner.key,
                &mint_owner.key,
            )?,
            &[
                token_program_account.clone(),
                rent_sysvar_info.clone(),
                mint_owner.clone(),
                member_account.clone(),
            ],
            &[&member_signer_seeds],
        )?;
        // mint the tokens to the account
        invoke_signed(
            &spl_token::instruction::mint_to(
                &spl_token::id(),
                &mint_owner.key,
                &member_pda,
                &mint_owner.key,
                &[],
                allocation_table[member_index as usize],
            )?,
            &[
                member_account.clone(),
                token_program_account.clone(),
                mint_owner.clone(),
                rent_sysvar_info.clone(),
            ],
            &[&mint_signer_seeds],
        )?;
    }
    squad_info.open = false;

    Squad::pack(squad_info, &mut squad_account.data.borrow_mut())?;
    Ok(())
}
