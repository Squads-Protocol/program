use solana_program::{
    account_info::{next_account_info, AccountInfo},
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
    rent::Rent,
    system_instruction::{allocate, assign, create_account, transfer},
    sysvar::Sysvar,
};

use num_traits::FromPrimitive;

use crate::state::squad::AllocationType;
use crate::{
    state::{
        proposal::{Proposal, ProposalType},
        squad::Squad,
    },
    *, // error::SquadError
};

// creates an account for the proposal
pub fn process_create_proposal(
    accounts: &[AccountInfo],
    proposal_type: u8,
    votes_num: u8,
    title: String,
    description: String,
    link: String,
    vote_labels: Vec<String>,
    start_timestamp: i64,
    close_timestamp: i64,
    amount: u64,
    minimum_out: u64,
    program_id: &Pubkey,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let initializer = next_account_info(account_info_iter)?;
    let squad_account = next_account_info(account_info_iter)?;
    // the soon to be proposal account generated as PDA on client side from nonce
    let proposal_account = next_account_info(account_info_iter)?;
    let system_account = next_account_info(account_info_iter)?;
    let rent_sysvar_info = next_account_info(account_info_iter)?;
    let squads_program_account = next_account_info(account_info_iter)?;

    let rent = &Rent::from_account_info(rent_sysvar_info)?;

    if !initializer.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let mut squad_account_info = get_squad(program_id, squad_account)?;

    // check that the submitted squads program account is actually this one
    if squads_program_account.key != program_id {
        return Err(ProgramError::IncorrectProgramId);
    }

    if !proposal_account.data_is_empty() {
        msg!("SQDS: This proposal has already been created");
        return Err(ProgramError::AccountAlreadyInitialized);
    }
    if !Squad::member_exists(&squad_account_info, initializer.key) {
        return Err(ProgramError::InvalidAccountData);
    }
    // check squad is not a draft/open
    if squad_account_info.open {
        return Err(ProgramError::InvalidInstructionData);
    }

    let proposal_nonce = squad_account_info.proposal_nonce + 1;
    let (proposal_address, proposal_bump_seed) =
        get_proposal_address_with_seed(&squad_account.key, &program_id, &proposal_nonce);

    // check that this is the proper sequential address
    if proposal_account.key != &proposal_address {
        msg!("SQDS Proposal nonce mismatch");
        return Err(ProgramError::InvalidAccountData);
    }

    let proposal_signer_seeds: &[&[_]] = &[
        &squad_account.key.to_bytes(),
        &proposal_nonce.to_le_bytes(),
        b"!proposal",
        &[proposal_bump_seed],
    ];

    // DoS check
    let rent_exempt_lamports = rent.minimum_balance(Proposal::get_packed_len()).max(1);
    if proposal_account.lamports() > 0 {
        let top_up_lamports = rent_exempt_lamports.saturating_sub(proposal_account.lamports());

        if top_up_lamports > 0 {
            invoke(
                &transfer(initializer.key, proposal_account.key, top_up_lamports),
                &[
                    initializer.clone(),
                    proposal_account.clone(),
                    system_account.clone(),
                ],
            )?;
        }

        invoke_signed(
            &allocate(proposal_account.key, Proposal::get_packed_len() as u64),
            &[proposal_account.clone(), system_account.clone()],
            &[&proposal_signer_seeds],
        )?;

        invoke_signed(
            &assign(proposal_account.key, program_id),
            &[proposal_account.clone(), system_account.clone()],
            &[&proposal_signer_seeds],
        )?;
    } else {
        invoke_signed(
            &create_account(
                initializer.key,
                &proposal_address,
                rent_exempt_lamports,
                Proposal::get_packed_len() as u64,
                &program_id,
            ),
            &[
                initializer.clone(),
                proposal_account.clone(),
                system_account.clone(),
            ],
            &[&proposal_signer_seeds],
        )?;
    }

    let mut proposal_account_info = get_proposal(program_id, squad_account, proposal_account)?;
    if proposal_account_info.is_initialized() {
        return Err(ProgramError::AccountAlreadyInitialized);
    }

    let actual_timestamp = Clock::get().unwrap().unix_timestamp;

    if proposal_type != ProposalType::Text as u8 {
        if votes_num != 2 {
            return Err(ProgramError::InvalidArgument);
        }
    }

    match FromPrimitive::from_u8(proposal_type) {
        Some(ProposalType::Text) => {
            // text
            if squad_account_info.allocation_type == AllocationType::Multisig as u8 {
                return Err(ProgramError::InvalidArgument);
            }
            Proposal::save_text(
                &mut proposal_account_info,
                proposal_type,
                title,
                description,
                link,
                initializer.key,
                votes_num,
                squad_account.key,
                vote_labels,
                if squad_account_info.allocation_type == AllocationType::Multisig as u8 {
                    actual_timestamp
                } else {
                    start_timestamp
                },
                close_timestamp,
                actual_timestamp,
                proposal_nonce,
            );
        }
        Some(ProposalType::Support) => {
            // support
            if squad_account_info.allocation_type == AllocationType::Multisig as u8 {
                return Err(ProgramError::InvalidArgument);
            }
            // check that support is within bounds (as %)
            if amount < 1 || amount > 100 {
                return Err(ProgramError::InvalidArgument);
            }
            Proposal::save_core(
                &mut proposal_account_info,
                proposal_type,
                title,
                description,
                link,
                initializer.key,
                votes_num,
                squad_account.key,
                vote_labels,
                if squad_account_info.allocation_type == AllocationType::Multisig as u8 {
                    actual_timestamp
                } else {
                    start_timestamp
                },
                close_timestamp,
                Clock::get().unwrap().unix_timestamp,
                amount,
                proposal_nonce,
            );
        }
        Some(ProposalType::Quorum) => {
            let amount_check = match squad_account_info.allocation_type {
                2 => {
                    // MS quorum amount is limited by max members
                    (squad_account_info.members.len() as u8) >= amount as u8
                }
                1 => {
                    // TS Quorum is limited to a percent
                    amount > 0 || amount <= 100
                }
                _ => false,
            };

            if !amount_check {
                return Err(ProgramError::InvalidInstructionData);
            }
            // quorum | threshold
            Proposal::save_core(
                &mut proposal_account_info,
                proposal_type,
                title,
                description,
                link,
                initializer.key,
                votes_num,
                squad_account.key,
                vote_labels,
                if squad_account_info.allocation_type == AllocationType::Multisig as u8 {
                    actual_timestamp
                } else {
                    start_timestamp
                },
                close_timestamp,
                Clock::get().unwrap().unix_timestamp,
                amount,
                proposal_nonce,
            );
        }
        Some(ProposalType::WithdrawSol) => {
            // withdraw SOL
            let source = next_account_info(account_info_iter)?;
            let target = next_account_info(account_info_iter)?;

            Proposal::save_withdraw(
                &mut proposal_account_info,
                proposal_type,
                title,
                description,
                link,
                source.key,
                target.key,
                initializer.key,
                votes_num,
                squad_account.key,
                vote_labels,
                if squad_account_info.allocation_type == AllocationType::Multisig as u8 {
                    actual_timestamp
                } else {
                    start_timestamp
                },
                close_timestamp,
                Clock::get().unwrap().unix_timestamp,
                amount,
                proposal_nonce,
            );
        }
        Some(ProposalType::WithdrawSpl) => {
            // withdraw token
            let source = next_account_info(account_info_iter)?;
            let target = next_account_info(account_info_iter)?;

            Proposal::save_withdraw(
                &mut proposal_account_info,
                proposal_type,
                title,
                description,
                link,
                source.key,
                target.key,
                initializer.key,
                votes_num,
                squad_account.key,
                vote_labels,
                if squad_account_info.allocation_type == AllocationType::Multisig as u8 {
                    actual_timestamp
                } else {
                    start_timestamp
                },
                close_timestamp,
                Clock::get().unwrap().unix_timestamp,
                amount,
                proposal_nonce,
            );
        }
        Some(ProposalType::AddMember) => {
            // add member
            let member = next_account_info(account_info_iter)?;

            Proposal::save_member(
                &mut proposal_account_info,
                proposal_type,
                title,
                description,
                link,
                member.key,
                initializer.key,
                votes_num,
                squad_account.key,
                vote_labels,
                if squad_account_info.allocation_type == AllocationType::Multisig as u8 {
                    actual_timestamp
                } else {
                    start_timestamp
                },
                close_timestamp,
                Clock::get().unwrap().unix_timestamp,
                amount,
                proposal_nonce,
            );
        }
        Some(ProposalType::RemoveMember) => {
            // remove member
            let member = next_account_info(account_info_iter)?;

            Proposal::save_member(
                &mut proposal_account_info,
                proposal_type,
                title,
                description,
                link,
                member.key,
                initializer.key,
                votes_num,
                squad_account.key,
                vote_labels,
                if squad_account_info.allocation_type == AllocationType::Multisig as u8 {
                    actual_timestamp
                } else {
                    start_timestamp
                },
                close_timestamp,
                Clock::get().unwrap().unix_timestamp,
                0,
                proposal_nonce,
            );
        }
        Some(ProposalType::MintMemberToken) => {
            // Mint member tokens
            if squad_account_info.allocation_type == AllocationType::Multisig as u8 {
                return Err(ProgramError::InvalidArgument);
            }
            let member = next_account_info(account_info_iter)?;

            Proposal::save_member(
                &mut proposal_account_info,
                proposal_type,
                title,
                description,
                link,
                member.key,
                initializer.key,
                votes_num,
                squad_account.key,
                vote_labels,
                if squad_account_info.allocation_type == AllocationType::Multisig as u8 {
                    actual_timestamp
                } else {
                    start_timestamp
                },
                close_timestamp,
                Clock::get().unwrap().unix_timestamp,
                amount,
                proposal_nonce,
            );
        }
        Some(ProposalType::Swap) => {
            // Swap
            let source = next_account_info(account_info_iter)?;
            let target = next_account_info(account_info_iter)?;

            Proposal::save_swap(
                &mut proposal_account_info,
                proposal_type,
                title,
                description,
                link,
                source.key,
                target.key,
                initializer.key,
                votes_num,
                squad_account.key,
                vote_labels,
                if squad_account_info.allocation_type == AllocationType::Multisig as u8 {
                    actual_timestamp
                } else {
                    start_timestamp
                },
                close_timestamp,
                Clock::get().unwrap().unix_timestamp,
                amount,
                minimum_out,
                proposal_nonce,
            );
        }
        None => {
            return Err(ProgramError::InvalidArgument);
        }
    }

    Proposal::pack(
        proposal_account_info,
        &mut proposal_account.data.borrow_mut(),
    )?;

    squad_account_info.proposal_nonce = proposal_nonce;

    Squad::pack(squad_account_info, &mut squad_account.data.borrow_mut())?;
    Ok(())
}
