/* SQUADS PROCESSOR */

mod process_add_members_to_squad;
mod process_cast_multisig_vote;
mod process_cast_vote;
mod process_create_multisig;
mod process_create_proposal;
mod process_create_squad;
mod process_execute_multisig_proposal;
mod process_execute_proposal;
mod process_execute_swap;
// mod process_quit_squad;

use process_add_members_to_squad::*;
use process_cast_multisig_vote::*;
use process_cast_vote::*;
use process_create_multisig::*;
use process_create_proposal::*;
use process_create_squad::*;
use process_execute_multisig_proposal::*;
use process_execute_proposal::*;
use process_execute_swap::*;
// use process_quit_squad::*;

use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey};

use crate::instruction::SquadInstruction;

pub type UnixTimestamp = i64;

pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let instruction = SquadInstruction::unpack(instruction_data)?;

    match instruction {
        SquadInstruction::CreateSquad {
            allocation_type,
            vote_support,
            vote_quorum,
            core_threshold,
            squad_name,
            description,
            token,
            random_id,
        } => process_create_squad(
            accounts,
            allocation_type,
            vote_support,
            vote_quorum,
            core_threshold,
            squad_name,
            description,
            token,
            random_id,
            program_id,
        ),

        SquadInstruction::CreateMultisig {
            vote_quorum,
            squad_name,
            description,
            random_id,
            members_num,
        } => process_create_multisig(
            accounts,
            vote_quorum,
            squad_name,
            description,
            random_id,
            members_num,
            program_id,
        ),

        SquadInstruction::AddMembersToSquad {
            members_num,
            allocation_table,
        } => process_add_members_to_squad(accounts, members_num, allocation_table, program_id),

        // Creat the proposal account
        SquadInstruction::CreateProposalAccount {
            proposal_type,
            votes_num,
            title,
            description,
            link,
            vote_labels,
            start_timestamp,
            close_timestamp,
            amount,
            minimum_out,
        } => process_create_proposal(
            accounts,
            proposal_type,
            votes_num,
            title,
            description,
            link,
            vote_labels,
            start_timestamp,
            close_timestamp,
            amount,
            minimum_out,
            program_id,
        ),

        // Proposal voting (private squad)
        SquadInstruction::CastVote { vote } => process_cast_vote(accounts, program_id, vote),

        // Proposal voting (private squad)
        SquadInstruction::ExecuteProposal { random_id } => {
            process_execute_proposal(accounts, random_id, program_id)
        }

        // Quitting a squad
        // SquadInstruction::QuitSquad => process_quit_squad(accounts, program_id),
        SquadInstruction::CastMultisigVote { vote } => {
            process_cast_multisig_vote(accounts, program_id, vote)
        }

        SquadInstruction::ExecuteMultisigProposal { random_id } => {
            process_execute_multisig_proposal(accounts, random_id, program_id)
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_text_proposal_vote_tipping() {
        // init mock data
        let token_supply = 100_000;

        // vote should tip
        fn run_tip_check(votes: &Vec<u64>, token_supply: u64) -> bool {
            let votes_copy = votes.clone();
            let total_votes = votes_copy.into_iter().reduce(|a, b| a + b).unwrap();
            let potential_votes_remaining = token_supply - total_votes;

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

            if votes[most_index] > votes[second_most_index] + potential_votes_remaining {
                return true;
            } else {
                return false;
            }
        }

        let tipping_vote_cases: Vec<Vec<u64>> = vec![
            vec![60_000, 0, 0, 0],
            vec![0, 60_000, 0, 0],
            vec![0, 0, 60_000, 0],
            vec![0, 20, 0, 60_000],
            vec![60_000, 10_000],
            vec![0, 30000, 0, 60_000],
            vec![10_000, 0, 0, 50_001],
            vec![100, 100, 100, 1000, 50_000],
        ];

        for test_votes in tipping_vote_cases.iter() {
            assert_eq!(run_tip_check(test_votes, token_supply), true)
        }

        let not_tipping_vote_cases: Vec<Vec<u64>> = vec![
            vec![50_000, 30_000],
            vec![0, 30_000],
            vec![10_000, 30_000],
            vec![10_000, 0, 0, 50_000],
            vec![0, 0, 0, 0, 0],
            vec![0, 0],
        ];

        for test_votes in not_tipping_vote_cases.iter() {
            assert_eq!(run_tip_check(test_votes, token_supply), false)
        }
    }
}
