use num_traits::FromPrimitive;
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    clock::Clock,
    entrypoint::ProgramResult,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
    system_instruction::transfer,
    sysvar::Sysvar,
};

use spl_associated_token_account::create_associated_token_account;

use crate::{
    state::{
        proposal::Proposal,
        squad::{Member, Squad},
    },
    *, // error::SquadError
};

use crate::processor::process_execute_swap;
use crate::state::proposal::ProposalType;
use crate::state::squad::AllocationType;

pub fn process_execute_multisig_proposal(
    accounts: &[AccountInfo],
    random_id: String,
    program_id: &Pubkey,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let executioner = next_account_info(account_info_iter)?;
    let squad_account = next_account_info(account_info_iter)?;
    let proposal_account = next_account_info(account_info_iter)?;
    let source_account = next_account_info(account_info_iter)?;
    let destination_account = next_account_info(account_info_iter)?;
    let system_program_account = next_account_info(account_info_iter)?;
    let token_program_account = next_account_info(account_info_iter)?;
    let associated_program_account = next_account_info(account_info_iter)?;
    let rent_account = next_account_info(account_info_iter)?;

    if !executioner.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let mut squad_account_info = get_squad(program_id, squad_account)?;
    let mut proposal_account_info = get_proposal(program_id, squad_account, proposal_account)?;

    // check the token program
    if *token_program_account.key != spl_token::id() {
        return Err(ProgramError::IncorrectProgramId);
    }
    // check the ata program
    if *associated_program_account.key != spl_associated_token_account::id() {
        return Err(ProgramError::InvalidAccountData);
    }
    // check that the source account also matches the proposal field
    if *source_account.key != proposal_account_info.execution_source {
        return Err(ProgramError::InvalidAccountData);
    }
    // check that the destination is the destination account from the proposal
    if *destination_account.key != proposal_account_info.execution_destination {
        return Err(ProgramError::InvalidAccountData);
    }

    if squad_account_info.allocation_type != AllocationType::Multisig as u8 {
        return Err(ProgramError::InvalidAccountData);
    }

    if !Squad::member_exists(&squad_account_info, executioner.key) {
        return Err(ProgramError::InvalidArgument);
    }

    // check if the proposal is actually executable
    if proposal_account_info.proposal_type < 1 {
        return Err(ProgramError::InvalidArgument);
    }

    // check if the proposal has already been executed
    if proposal_account_info.executed == true {
        return Err(ProgramError::InvalidArgument);
    }

    let pass_votes = *proposal_account_info.votes.get(0).unwrap();

    // check threshold
    let threshold_reached;
    if proposal_account_info.execute_ready {
        threshold_reached = pass_votes as f32 >= proposal_account_info.threshold_at_execute as f32;
    } else {
        threshold_reached = pass_votes as f32 >= squad_account_info.vote_quorum as f32;
    }

    if !threshold_reached {
        return Err(ProgramError::InvalidArgument);
    }

    match FromPrimitive::from_u8(proposal_account_info.proposal_type) {
        Some(ProposalType::Quorum) => {
            // change quorum (threshold)
            squad_account_info.vote_quorum = proposal_account_info.execution_amount as u8;
        }
        Some(ProposalType::WithdrawSol) => {
            // withdraw SOL
            // check the source account is the squad sol_acccount
            if source_account.key != &squad_account_info.sol_account {
                return Err(ProgramError::InvalidInstructionData);
            }
            let (sol_address, sol_bump_seed) =
                get_sol_address_with_seed(&squad_account.key, program_id);

            // check that the derived sol address is the source account
            if *source_account.key != sol_address {
                return Err(ProgramError::InvalidAccountData);
            }

            let sol_signer_seeds: &[&[_]] = &[
                &squad_account.key.to_bytes(),
                b"!squadsol",
                &[sol_bump_seed],
            ];
            let transfer_ix = transfer(
                &sol_address,
                &destination_account.key,
                proposal_account_info.execution_amount,
            );
            invoke_signed(
                &transfer_ix,
                &[
                    source_account.clone(),
                    destination_account.clone(),
                    system_program_account.clone(),
                ],
                &[&sol_signer_seeds],
            )?;
        }
        Some(ProposalType::WithdrawSpl) => {
            // withdraw token
            let destination_ata = next_account_info(account_info_iter)?;
            let token_mint = next_account_info(account_info_iter)?;
            let sol_account = next_account_info(account_info_iter)?;

            if sol_account.key != &squad_account_info.sol_account {
                return Err(ProgramError::InvalidInstructionData);
            }

            // check that the destination ata that was submitted matches the one
            // that would be derived from the proposal destination
            let ata_address = spl_associated_token_account::get_associated_token_address(
                &proposal_account_info.execution_destination,
                token_mint.key,
            );
            if ata_address != *destination_ata.key {
                return Err(ProgramError::InvalidAccountData);
            }

            let (sol_address, sol_bump_seed) =
                get_sol_address_with_seed(&squad_account.key, program_id);
            let sol_signer_seeds: &[&[_]] = &[
                &squad_account.key.to_bytes(),
                b"!squadsol",
                &[sol_bump_seed],
            ];

            if destination_ata.data_is_empty() {
                invoke(
                    &create_associated_token_account(
                        &executioner.key,
                        &destination_account.key,
                        &token_mint.key,
                    ),
                    &[
                        executioner.clone(),
                        destination_ata.clone(),
                        destination_account.clone(),
                        token_mint.clone(),
                        system_program_account.clone(),
                        token_program_account.clone(),
                        rent_account.clone(),
                        associated_program_account.clone(),
                    ],
                )?;
            }

            let token_transfer_ix = &spl_token::instruction::transfer(
                &token_program_account.key,
                &source_account.key,
                &destination_ata.key,
                &sol_address,
                &[],
                proposal_account_info.execution_amount,
            )?;

            invoke_signed(
                token_transfer_ix,
                &[
                    source_account.clone(),
                    destination_ata.clone(),
                    sol_account.clone(),
                    token_program_account.clone(),
                    system_program_account.clone(),
                ],
                &[&sol_signer_seeds],
            )?;
        }
        Some(ProposalType::AddMember) => {
            // add member
            if Squad::member_exists(&squad_account_info, destination_account.key) {
                return Err(ProgramError::InvalidArgument);
            }

            Squad::add_member(
                &mut squad_account_info,
                *destination_account.key,
                Member {
                    equity_token_account: *destination_account.key,
                },
            );
        }
        Some(ProposalType::RemoveMember) => {
            // remove member
            if !Squad::member_exists(&squad_account_info, destination_account.key) {
                return Err(ProgramError::InvalidArgument);
            }

            if squad_account_info.vote_quorum == squad_account_info.members.len() as u8 {
                squad_account_info.vote_quorum -= 1;
            }

            Squad::remove_member(&mut squad_account_info, &destination_account.key);
        }
        Some(ProposalType::Swap) => {
            // swap tokens
            let sol_account = next_account_info(account_info_iter)?;
            let source_account_ata = next_account_info(account_info_iter)?;
            let destination_account_ata = next_account_info(account_info_iter)?;
            let wsol_account = next_account_info(account_info_iter)?;
            let wsol_mint = next_account_info(account_info_iter)?;

            let (sol_address, _sol_bump_seed) =
                get_sol_address_with_seed(&squad_account.key, program_id);

            if sol_account.key != &sol_address {
                return Err(ProgramError::InvalidAccountData);
            }

            // unpack the proposal and squad
            let proposal_account_info =
                Proposal::unpack_unchecked(&proposal_account.data.borrow())?;

            if wsol_mint.key != &spl_token::native_mint::id() {
                return Err(ProgramError::InvalidAccountData);
            }

            // Check src_mint
            if *source_account.key != proposal_account_info.execution_source {
                return Err(ProgramError::InvalidAccountData);
            }
            // Check dest_mint
            if *destination_account.key != proposal_account_info.execution_destination {
                return Err(ProgramError::InvalidAccountData);
            }

            // Check ata src
            let mut ata_source = spl_associated_token_account::get_associated_token_address(
                &sol_address,
                &proposal_account_info.execution_source,
            );
            // Check if mint is SOL mint
            if proposal_account_info.execution_source == spl_token::native_mint::id() {
                ata_source = get_wsol_address(&sol_address, &random_id, program_id);

                if *wsol_account.key != ata_source {
                    return Err(ProgramError::InvalidAccountData);
                }
            }
            if ata_source != *source_account_ata.key {
                return Err(ProgramError::InvalidAccountData);
            }

            // Check ata dest
            let mut ata_destination = spl_associated_token_account::get_associated_token_address(
                &sol_address,
                &proposal_account_info.execution_destination,
            );
            // Check if mint is SOL mint
            if proposal_account_info.execution_destination == spl_token::native_mint::id() {
                ata_destination = get_wsol_address(&sol_address, &random_id, program_id);

                if *wsol_account.key != ata_destination {
                    return Err(ProgramError::InvalidAccountData);
                }
            }
            if ata_destination != *destination_account_ata.key {
                return Err(ProgramError::InvalidAccountData);
            }

            process_execute_swap(
                accounts,
                proposal_account_info.execution_amount,
                proposal_account_info.execution_amount_out,
                squad_account_info.allocation_type,
                random_id,
                program_id,
            )?;
        }
        _ => {
            return Err(ProgramError::InvalidArgument);
        }
    };

    proposal_account_info.executed_by = *executioner.key;
    proposal_account_info.executed = true;
    proposal_account_info.execution_date = Clock::get().unwrap().unix_timestamp;
    Proposal::pack(
        proposal_account_info,
        &mut proposal_account.data.borrow_mut(),
    )?;
    Squad::pack(squad_account_info, &mut squad_account.data.borrow_mut())?;
    Ok(())
}
