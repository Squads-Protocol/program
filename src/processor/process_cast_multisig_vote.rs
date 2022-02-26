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

use crate::state::squad::AllocationType;
use crate::{
    state::{proposal::Proposal, squad::Squad, vote::VoteReceipt},
    *,
};

pub fn process_cast_multisig_vote(
    accounts: &[AccountInfo],
    program_id: &Pubkey,
    vote: u8,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let initializer = next_account_info(account_info_iter)?;
    let squad_account = next_account_info(account_info_iter)?;
    let proposal_account = next_account_info(account_info_iter)?;
    let vote_account = next_account_info(account_info_iter)?;
    let system_program_account = next_account_info(account_info_iter)?;
    let rent_account = next_account_info(account_info_iter)?;
    let squads_program_account = next_account_info(account_info_iter)?;
    let rent = &Rent::from_account_info(rent_account)?;

    if !initializer.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // check the submitted program id
    if squads_program_account.key != program_id {
        return Err(ProgramError::InvalidAccountData);
    }

    // unpack the squad account
    let squad_account_info = get_squad(program_id, squad_account)?;
    let mut proposal_info = get_proposal(program_id, squad_account, proposal_account)?;

    if proposal_info.close_timestamp < Clock::get().unwrap().unix_timestamp {
        msg!("SQDS: Vote rejected, proposal has already ended");
        return Err(ProgramError::InvalidArgument);
    }

    if proposal_info.start_timestamp > Clock::get().unwrap().unix_timestamp {
        msg!("SQDS: Vote rejected, proposal has not started yet");
        return Err(ProgramError::InvalidArgument);
    }

    if proposal_info.executed {
        msg!("SQDS: Vote rejected, proposal has already executed");
        return Err(ProgramError::InvalidArgument);
    }

    // check if this is a multisig
    if squad_account_info.allocation_type != AllocationType::Multisig as u8 {
        return Err(ProgramError::InvalidArgument);
    }
    // check if the voter is a member
    if !Squad::member_exists(&squad_account_info, initializer.key) {
        return Err(ProgramError::InvalidArgument);
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

    let mut vote_account_info = get_vote(program_id, squad_account, vote_account)?;

    VoteReceipt::save_vote(
        &mut vote_account_info,
        proposal_account.key,
        vote,
        initializer.key,
        Clock::get().unwrap().unix_timestamp,
        1,
    );

    VoteReceipt::pack(vote_account_info, &mut vote_account.data.borrow_mut())?;

    // record the vote to the proposal
    let curr_vote = proposal_info.votes.get_mut(vote as usize).unwrap();
    *curr_vote += 1;
    proposal_info.has_voted.push(*initializer.key);
    proposal_info.has_voted_num = proposal_info.has_voted.len() as u8;

    let mut quorum_ready = false;

    let pass_votes = *proposal_info.votes.get(0).unwrap();
    let fail_votes = *proposal_info.votes.get(1).unwrap();
    let possible_votes_left = squad_account_info.members.len() as u64 - (pass_votes + fail_votes);

    if squad_account_info.vote_quorum as u64 > (possible_votes_left + pass_votes) {
        proposal_info.execute_ready = true;
        proposal_info.executed = true;
    }

    if pass_votes as f32 >= squad_account_info.vote_quorum as f32 {
        quorum_ready = true;
    }

    if quorum_ready {
        proposal_info.execute_ready = true;
    }

    if proposal_info.execute_ready {
        proposal_info.threshold_at_execute = squad_account_info.vote_quorum;
    }

    Proposal::pack(proposal_info, &mut proposal_account.data.borrow_mut())?;
    Ok(())
}
