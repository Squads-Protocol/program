use num_traits::FromPrimitive;
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
use spl_token::{
    instruction::initialize_account,
    state::{Account, Mint},
};

use spl_associated_token_account::create_associated_token_account;

use crate::{
    state::{
        proposal::Proposal,
        squad::{Member, Squad},
    },
    *,
};

use crate::processor::process_execute_swap;
use crate::state::proposal::ProposalType;
use crate::state::squad::AllocationType;

pub fn process_execute_proposal(
    accounts: &[AccountInfo],
    random_id: String,
    program_id: &Pubkey,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let executioner = next_account_info(account_info_iter)?;
    let squad_account = next_account_info(account_info_iter)?;
    let squad_mint_account = next_account_info(account_info_iter)?;
    let proposal_account = next_account_info(account_info_iter)?;
    let source_account = next_account_info(account_info_iter)?;
    let destination_account = next_account_info(account_info_iter)?;
    let system_program_account = next_account_info(account_info_iter)?;
    let token_program_account = next_account_info(account_info_iter)?;
    let associated_program_account = next_account_info(account_info_iter)?;
    let rent_account = next_account_info(account_info_iter)?;
    let rent = &Rent::from_account_info(rent_account)?;

    if !executioner.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // unpack the proposal and squad
    let mut squad_account_info = get_squad(program_id, squad_account)?;
    let mut proposal_account_info = get_proposal(program_id, squad_account, proposal_account)?;

    // check there isn't a member change lock on this proposal
    // if this proposal index is less than the member_lock_index, no voting allowed
    if !proposal_account_info.execute_ready
        && proposal_account_info.proposal_index <= squad_account_info.member_lock_index
    {
        return Err(ProgramError::InvalidInstructionData);
    }

    // check that the squad mint belongs to the squad
    if squad_account_info.mint_address != *squad_mint_account.key {
        return Err(ProgramError::InvalidAccountData);
    }
    // check the token program
    if *token_program_account.key != spl_token::id() {
        return Err(ProgramError::IncorrectProgramId);
    }
    // check that the squad mint owner is the token program id
    if *squad_mint_account.owner != spl_token::id() {
        return Err(ProgramError::InvalidAccountData);
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
    // check that this is a Team Coordinated squad, not a multisig
    if squad_account_info.allocation_type != AllocationType::TeamCoordination as u8 {
        return Err(ProgramError::InvalidAccountData);
    }
    // check that the person executing is a member of the squad
    if !Squad::member_exists(&squad_account_info, executioner.key) {
        return Err(ProgramError::InvalidArgument);
    }
    // check if the proposal is actually executeable
    if proposal_account_info.proposal_type < 1 {
        return Err(ProgramError::InvalidArgument);
    }
    // check if the proposal has already been executed
    if proposal_account_info.executed == true {
        return Err(ProgramError::InvalidArgument);
    }

    let mut vote_passed = true;
    // there are only two viable options for executable proposals
    // 0 pass, 1 reject
    let pass_votes = *proposal_account_info.votes.get(0).unwrap();
    let fail_votes = *proposal_account_info.votes.get(1).unwrap();
    if pass_votes < fail_votes {
        vote_passed = false;
    }

    if !vote_passed {
        return Err(ProgramError::InvalidArgument);
    }

    // get mint account supply
    let squad_mint_account_info = Mint::unpack_unchecked(&squad_mint_account.data.borrow())?;

    // check quorum & support
    let curr_quorum_percent;
    let current_support_percent;
    if proposal_account_info.execute_ready {
        curr_quorum_percent = (proposal_account_info.has_voted.len() as f32
            / proposal_account_info.members_at_execute as f32)
            * 100.0;

        current_support_percent =
            (pass_votes as f32 / proposal_account_info.supply_at_execute as f32) * 100.0;
    } else {
        curr_quorum_percent = (proposal_account_info.has_voted.len() as f32
            / squad_account_info.members.len() as f32)
            * 100.0;

        current_support_percent =
            (pass_votes as f32 / squad_mint_account_info.supply as f32) * 100.0;
    }

    if curr_quorum_percent < squad_account_info.vote_quorum as f32 {
        return Err(ProgramError::InvalidArgument);
    }

    if current_support_percent < squad_account_info.vote_support as f32 {
        return Err(ProgramError::InvalidArgument);
    }

    match FromPrimitive::from_u8(proposal_account_info.proposal_type) {
        Some(ProposalType::Support) => {
            // change support
            squad_account_info.vote_support = proposal_account_info.execution_amount as u8;
        }
        Some(ProposalType::Quorum) => {
            // change quorum
            squad_account_info.vote_quorum = proposal_account_info.execution_amount as u8;
        }
        Some(ProposalType::WithdrawSol) => {
            // withdraw SOL

            let (sol_address, sol_bump_seed) =
                get_sol_address_with_seed(&squad_account.key, program_id);

            // check that the sol_address matches the squad sol address
            if sol_address != squad_account_info.sol_account {
                return Err(ProgramError::InvalidAccountData);
            }

            // check that the sold_address matches the source account
            if sol_address != proposal_account_info.execution_source {
                return Err(ProgramError::InvalidAccountData);
            }

            // get the pda seeds
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
            // the destination account is a normal token account, but
            // we will be sending to an ATA
            let destination_ata = next_account_info(account_info_iter)?;
            let token_mint = next_account_info(account_info_iter)?;
            let sol_account = next_account_info(account_info_iter)?;
            // get the sol address and seed, as this is still the authority over any
            // of the squads token accounts - they're derived from this
            let (sol_address, sol_bump_seed) =
                get_sol_address_with_seed(&squad_account.key, program_id);

            // check that the derived sol PDA (sol_address) matches the squad
            if sol_address != squad_account_info.sol_account {
                return Err(ProgramError::InvalidAccountData);
            }
            // check that the sol account is the one set to the squad account
            if sol_account.key != &squad_account_info.sol_account {
                return Err(ProgramError::InvalidInstructionData);
            }
            // check that the destination ata that was submitted matches the one that would be derived
            let ata_address = spl_associated_token_account::get_associated_token_address(
                &proposal_account_info.execution_destination,
                token_mint.key,
            );
            if ata_address != *destination_ata.key {
                return Err(ProgramError::InvalidAccountData);
            }

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
            let mint_owner = next_account_info(account_info_iter)?;
            let member_account = next_account_info(account_info_iter)?;

            let (mint_owner_address, mint_bump_seed) =
                get_mint_address_with_seed(&squad_account.key, &program_id);
            if mint_owner_address != *mint_owner.key {
                // msg!("Error: mint address derivation mismatch");
                return Err(ProgramError::InvalidArgument);
            }

            let mint_signer_seeds: &[&[_]] = &[
                &squad_account.key.to_bytes(),
                b"!squadmint",
                &[mint_bump_seed],
            ];

            let (member_pda, member_bump_seed) = get_equity_address_with_seed(
                &proposal_account_info.execution_destination,
                squad_account.key,
                program_id,
            );

            // check that derived member_pda from execution_destination matches new member pda
            if member_pda != *member_account.key {
                return Err(ProgramError::InvalidAccountData);
            }

            let member_signer_seeds: &[&[_]] = &[
                &destination_account.key.to_bytes(),
                &squad_account.key.to_bytes(),
                b"!memberequity",
                &[member_bump_seed],
            ];

            // DoS check
            let rent_exempt_lamports = rent
                .minimum_balance(spl_token::state::Account::get_packed_len())
                .max(1);
            if member_account.lamports() > 0 {
                let top_up_lamports =
                    rent_exempt_lamports.saturating_sub(member_account.lamports());

                if top_up_lamports > 0 {
                    invoke(
                        &transfer(executioner.key, member_account.key, top_up_lamports),
                        &[
                            executioner.clone(),
                            member_account.clone(),
                            system_program_account.clone(),
                        ],
                    )?;
                }

                invoke_signed(
                    &allocate(
                        member_account.key,
                        spl_token::state::Account::get_packed_len() as u64,
                    ),
                    &[member_account.clone(), system_program_account.clone()],
                    &[&member_signer_seeds],
                )?;

                invoke_signed(
                    &assign(member_account.key, &spl_token::id()),
                    &[member_account.clone(), system_program_account.clone()],
                    &[&member_signer_seeds],
                )?;
            } else {
                // create the equity token account for the member
                invoke_signed(
                    &create_account(
                        executioner.key,
                        &member_pda,
                        1.max(rent.minimum_balance(spl_token::state::Account::get_packed_len())),
                        spl_token::state::Account::get_packed_len() as u64,
                        &spl_token::id(),
                    ),
                    &[
                        executioner.clone(),
                        member_account.clone(),
                        system_program_account.clone(),
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
                    rent_account.clone(),
                    mint_owner.clone(),
                    member_account.clone(),
                ],
                &[&member_signer_seeds],
            )?;

            invoke_signed(
                &spl_token::instruction::mint_to(
                    &spl_token::id(),
                    &mint_owner.key,
                    &member_pda,
                    &mint_owner.key,
                    &[],
                    proposal_account_info.execution_amount,
                )?,
                &[
                    member_account.clone(),
                    token_program_account.clone(),
                    mint_owner.clone(),
                    rent_account.clone(),
                ],
                &[&mint_signer_seeds],
            )?;

            Squad::add_member(
                &mut squad_account_info,
                *destination_account.key,
                Member {
                    equity_token_account: *member_account.key,
                },
            );
        }
        Some(ProposalType::RemoveMember) => {
            // remove member
            if !Squad::member_exists(&squad_account_info, destination_account.key) {
                return Err(ProgramError::InvalidArgument);
            }

            let mint_owner = next_account_info(account_info_iter)?;
            let member_account = next_account_info(account_info_iter)?;
            let sol_account = next_account_info(account_info_iter)?;

            if sol_account.key != &squad_account_info.sol_account {
                return Err(ProgramError::InvalidInstructionData);
            }

            let (mint_owner_address, mint_bump_seed) =
                get_mint_address_with_seed(&squad_account.key, &program_id);
            if mint_owner_address != *mint_owner.key {
                return Err(ProgramError::InvalidArgument);
            }

            let mint_signer_seeds: &[&[_]] = &[
                &squad_account.key.to_bytes(),
                b"!squadmint",
                &[mint_bump_seed],
            ];

            let (member_pda, _member_bump_seed) = get_equity_address_with_seed(
                destination_account.key,
                squad_account.key,
                program_id,
            );

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

            Squad::remove_member(&mut squad_account_info, &destination_account.key);

            // change in membership, lock the proposals
            squad_account_info.member_lock_index = squad_account_info.proposal_nonce;
        }
        Some(ProposalType::MintMemberToken) => {
            // change member mint allocation
            if !Squad::member_exists(&squad_account_info, destination_account.key) {
                return Err(ProgramError::InvalidArgument);
            }
            let member_equity =
                get_equity_address(destination_account.key, squad_account.key, program_id);

            let mint_owner = next_account_info(account_info_iter)?;
            let member_account = next_account_info(account_info_iter)?;

            if *member_account.key != member_equity {
                return Err(ProgramError::InvalidAccountData);
            }

            let (mint_owner_address, mint_bump_seed) =
                get_mint_address_with_seed(&squad_account.key, &program_id);
            if mint_owner_address != *mint_owner.key {
                return Err(ProgramError::InvalidAccountData);
            }

            let member_account_info = Account::unpack_unchecked(&member_account.data.borrow())?;

            proposal_account_info.execution_amount_out = member_account_info.amount;

            let mint_signer_seeds: &[&[_]] = &[
                &squad_account.key.to_bytes(),
                b"!squadmint",
                &[mint_bump_seed],
            ];

            if member_account_info.amount < proposal_account_info.execution_amount {
                invoke_signed(
                    &spl_token::instruction::mint_to(
                        &spl_token::id(),
                        &mint_owner.key,
                        &member_account.key,
                        &mint_owner.key,
                        &[],
                        proposal_account_info.execution_amount - member_account_info.amount,
                    )?,
                    &[
                        member_account.clone(),
                        token_program_account.clone(),
                        mint_owner.clone(),
                        rent_account.clone(),
                    ],
                    &[&mint_signer_seeds],
                )?;
                // change in ownership values, set the member lock
                squad_account_info.member_lock_index = squad_account_info.proposal_nonce;
            } else if member_account_info.amount > proposal_account_info.execution_amount {
                invoke_signed(
                    &spl_token::instruction::burn(
                        &spl_token::id(),
                        &member_account.key,
                        &mint_owner.key,
                        &mint_owner.key,
                        &[],
                        member_account_info.amount - proposal_account_info.execution_amount,
                    )?,
                    &[
                        member_account.clone(),
                        token_program_account.clone(),
                        mint_owner.clone(),
                    ],
                    &[&mint_signer_seeds],
                )?;
                // change in ownership, set the member lock
                squad_account_info.member_lock_index = squad_account_info.proposal_nonce;
            }
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
            msg!("SQDS: Invalid execution: execution type not found.");
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
