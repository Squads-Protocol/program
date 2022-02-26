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
use spl_token::state::{Account, Mint};

use crate::state::proposal::ProposalType;
use crate::state::squad::AllocationType;
use crate::{
    state::{proposal::Proposal, squad::Squad, vote::VoteReceipt},
    *,
};

pub fn process_cast_vote(accounts: &[AccountInfo], program_id: &Pubkey, vote: u8) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let initializer = next_account_info(account_info_iter)?;
    let squad_account = next_account_info(account_info_iter)?;
    let squad_mint_account = next_account_info(account_info_iter)?;
    let proposal_account = next_account_info(account_info_iter)?;
    let member_governance_account = next_account_info(account_info_iter)?;
    let vote_account = next_account_info(account_info_iter)?;
    let system_program_account = next_account_info(account_info_iter)?;
    let rent_account = next_account_info(account_info_iter)?;
    let squads_program_account = next_account_info(account_info_iter)?;
    let rent = &Rent::from_account_info(rent_account)?;

    // check that the signer
    if !initializer.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    // check that the squad mint owner is the token program id
    if *squad_mint_account.owner != spl_token::id() {
        msg!(
            "SQDS: Mint not owned by token program {:?}",
            squad_mint_account.owner
        );
        return Err(ProgramError::InvalidAccountData);
    }
    // check that the submitted squads program account is actually this one
    if squads_program_account.key != program_id {
        return Err(ProgramError::IncorrectProgramId);
    }

    // unpack the squad account
    let squad_account_info = get_squad(program_id, squad_account)?;
    // unpack for the data struct and for additional checks
    let mut proposal_info = get_proposal(program_id, squad_account, proposal_account)?;

    // check if this is a multisig
    if squad_account_info.allocation_type != AllocationType::TeamCoordination as u8 {
        return Err(ProgramError::InvalidArgument);
    }

    // check there isn't a member change lock on this proposal
    // if this proposal index is less than the member_lock_index, no voting allowed
    if !proposal_info.execute_ready
        && proposal_info.proposal_index <= squad_account_info.member_lock_index
    {
        return Err(ProgramError::InvalidInstructionData);
    }

    //check that the squad_mint is for this squad
    if *squad_mint_account.key != squad_account_info.mint_address {
        msg!("SQDS: Incorrect squad mint address");
        return Err(ProgramError::InvalidAccountData);
    }

    // check that this proposal isnt closed
    if proposal_info.close_timestamp < Clock::get().unwrap().unix_timestamp {
        msg!("SQDS: Vote rejected, proposal has already ended");
        return Err(ProgramError::InvalidArgument);
    }

    // check that this proposal has started
    if proposal_info.start_timestamp > Clock::get().unwrap().unix_timestamp {
        msg!("SQDS: Vote rejected, proposal has not started yet");
        return Err(ProgramError::InvalidArgument);
    }

    if proposal_info.executed {
        msg!("SQDS: Vote rejected, proposal has already executed");
        return Err(ProgramError::InvalidArgument);
    }

    // check that the signer is a member of this squad
    if !Squad::member_exists(&squad_account_info, initializer.key) {
        return Err(ProgramError::InvalidArgument);
    }

    let member_governance_address =
        get_equity_address(initializer.key, squad_account.key, program_id);

    // check that the derived governance address for this user actually matches the submitted one
    if member_governance_address != *member_governance_account.key {
        msg!("SQDS: Invalid member governance address");
        return Err(ProgramError::InvalidAccountData);
    }

    let (vote_address, vote_bump) =
        get_vote_address_with_seed(&proposal_account.key, program_id, &initializer.key);

    let seedstring = String::from("!vote");
    let vote_signer_seeds: &[&[_]] = &[
        &proposal_account.key.to_bytes(),
        &initializer.key.to_bytes(),
        &seedstring.as_bytes(),
        &[vote_bump],
    ];
    // check that the vote account PDA is correct
    if vote_address != *vote_account.key {
        msg!("SQDS: Vote account PDA mismatch");
        return Err(ProgramError::InvalidAccountData);
    }

    if !vote_account.data_is_empty() {
        msg!("SQDS: Vote already exists for this member");
        return Err(ProgramError::AccountAlreadyInitialized);
    }

    if vote >= proposal_info.votes_num {
        return Err(ProgramError::InvalidArgument);
    }

    // DoS check
    let rent_exempt_lamports = rent.minimum_balance(VoteReceipt::get_packed_len()).max(1);
    if vote_account.lamports() > 0 {
        let top_up_lamports = rent_exempt_lamports.saturating_sub(vote_account.lamports());

        if top_up_lamports > 0 {
            invoke(
                &transfer(initializer.key, vote_account.key, top_up_lamports),
                &[
                    initializer.clone(),
                    vote_account.clone(),
                    system_program_account.clone(),
                ],
            )?;
        }

        invoke_signed(
            &allocate(vote_account.key, VoteReceipt::get_packed_len() as u64),
            &[vote_account.clone(), system_program_account.clone()],
            &[&vote_signer_seeds],
        )?;

        invoke_signed(
            &assign(vote_account.key, program_id),
            &[vote_account.clone(), system_program_account.clone()],
            &[&vote_signer_seeds],
        )?;
    } else {
        invoke_signed(
            &create_account(
                initializer.key,
                &vote_address,
                rent_exempt_lamports,
                VoteReceipt::get_packed_len() as u64,
                &program_id,
            ),
            &[
                initializer.clone(),
                vote_account.clone(),
                system_program_account.clone(),
            ],
            &[&vote_signer_seeds],
        )?;
    }

    let governance_account_info =
        Account::unpack_unchecked(&member_governance_account.data.borrow())?;

    let mut vote_account_info = get_vote(program_id, squad_account, vote_account)?;

    VoteReceipt::save_vote(
        &mut vote_account_info,
        proposal_account.key,
        vote,
        initializer.key,
        Clock::get().unwrap().unix_timestamp,
        governance_account_info.amount,
    );

    VoteReceipt::pack(vote_account_info, &mut vote_account.data.borrow_mut())?;

    // record the vote to the proposal
    let curr_vote = proposal_info.votes.get_mut(vote as usize).unwrap();
    *curr_vote += governance_account_info.amount;
    proposal_info.has_voted.push(*initializer.key);
    proposal_info.has_voted_num = proposal_info.has_voted.len() as u8;

    // get mint account supply
    let squad_mint_account_info = Mint::unpack_unchecked(&squad_mint_account.data.borrow())?;

    let total_votes_copy = proposal_info.votes.clone();
    let total_votes = total_votes_copy.into_iter().reduce(|a, b| a + b).unwrap();

    // get total votes
    let possible_votes_left = squad_mint_account_info.supply - total_votes;

    if proposal_info.proposal_type == ProposalType::Text as u8 {
        let votes = proposal_info.votes.clone();
        let most_index = votes
            .iter()
            .enumerate()
            .fold(
                (0, 0),
                |max, (ind, &val)| if val > max.1 { (ind, val) } else { max },
            )
            .0;
        let second_most_index = votes
            .iter()
            .enumerate()
            .fold((0, 0), |max, (ind, &val)| {
                if ind == most_index {
                    if most_index == 0 {
                        (ind + 1, 0)
                    } else {
                        max
                    }
                } else if val > max.1 {
                    (ind, val)
                } else {
                    max
                }
            })
            .0;

        if votes[most_index] > votes[second_most_index] + possible_votes_left {
            let mut quorum_ready = false;
            let curr_quorum_percent = (proposal_info.has_voted.len() as f32
                / squad_account_info.members.len() as f32)
                * 100.0;

            if curr_quorum_percent >= squad_account_info.vote_quorum as f32 {
                quorum_ready = true;
            }

            let mut support_ready = false;
            let current_support_percent =
                (votes[most_index] as f32 / squad_mint_account_info.supply as f32) * 100.0;
            if current_support_percent >= squad_account_info.vote_support as f32 {
                support_ready = true;
            }

            if quorum_ready && support_ready {
                proposal_info.execute_ready = true;
            }
        }
    } else {
        let pass_votes = *proposal_info.votes.get(0).unwrap();
        let fail_votes = *proposal_info.votes.get(1).unwrap();

        // Close proposal if decline are greater than accept
        if fail_votes > pass_votes + possible_votes_left {
            proposal_info.executed = true;
        }

        // check quorum
        let mut quorum_ready = false;
        let curr_quorum_percent = (proposal_info.has_voted.len() as f32
            / squad_account_info.members.len() as f32)
            * 100.0;

        if curr_quorum_percent >= squad_account_info.vote_quorum as f32 {
            quorum_ready = true;
        }

        // check support
        let mut support_ready = false;
        let current_support_percent =
            (pass_votes as f32 / squad_mint_account_info.supply as f32) * 100.0;
        if current_support_percent >= squad_account_info.vote_support as f32 {
            support_ready = true;
        }

        if quorum_ready && support_ready {
            proposal_info.execute_ready = true;
        }
    }

    // Save supply at execute & members to have history on each proposal/vote
    proposal_info.supply_at_execute = squad_mint_account_info.supply;
    proposal_info.members_at_execute = squad_account_info.members.len() as u8;

    Proposal::pack(proposal_info, &mut proposal_account.data.borrow_mut())?;
    Ok(())
}
