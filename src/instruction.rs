/* SQUADS */

// main "API of the Squad Program"

use solana_program::{program_error::ProgramError, pubkey::Pubkey};
use std::convert::TryInto;

use borsh::{BorshDeserialize, BorshSerialize};

use crate::error::SquadError::InvalidInstruction;

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]

pub struct IncomingMember {
    pub public_key: Pubkey,
    pub equity_token_account: Pubkey, // contributions_account: [u8; 32], // need to expand for each mint
}

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
pub struct Members {
    pub list: Vec<IncomingMember>,
}

pub type UnixTimestamp = i64;

#[derive(Debug)]
pub enum SquadInstruction {
    /// ACCOUNTS EXPECTED
    /// 1. [signer] - the user creating the squad/payer/initial member
    /// 2. [writable] - the account address for the new squad
    /// 3. [] - the mint address for the new squad
    /// 4. [] - the member marker mint account
    /// 5. [] - the token program account
    /// 6. [] - the system program account
    /// 7. [] - the rent sysvar account
    CreateSquad {
        allocation_type: u8,
        vote_support: u8,
        vote_quorum: u8,
        core_threshold: u8,
        squad_name: String,
        description: String,
        token: String,
        random_id: String,
    },

    /// ACCOUNTS EXPECTED
    /// 1. [signer] - the user creating the squad/payer/initial member
    /// 2. [writable] - the account address for the new squad
    /// 3. [] - the system program account
    /// 4. [] - the rent sysvar account
    /// 5. [] - the squad program account
    /// 6. [...] - initial membes key
    CreateMultisig {
        vote_quorum: u8,
        squad_name: String,
        description: String,
        random_id: String,
        members_num: u8,
    },

    /// ACCOUNTS EXPECTED
    /// 1. [signer] - the user creating the squad/payer/initial member
    /// 2. [writable] - the account address for the new squad
    /// 3. [] - the mint address for the new squad
    /// 4. [] - the member marker mint account
    /// 5. [] - the token program account
    /// 6. [] - the system program account
    /// 7. [] - the rent sysvar account
    /// 8. [...] - the keys of the members being added
    AddMembersToSquad {
        members_num: u8,
        allocation_table: Vec<u64>,
    },

    /// ACCOUNTS EXPECTED
    /// 1. [signer] - the signer of the transaction, and the wallet address of the squad member
    /// 2. [writable] - the squad account
    /// 3. [] - the proposal account (PDA)
    /// 4. [] - the system account
    /// 5. [] - the rent sys var account
    /// 6. [] - the squad program account
    CreateProposalAccount {
        proposal_type: u8,
        votes_num: u8,
        title: String,
        description: String,
        link: String,
        vote_labels: Vec<String>,
        start_timestamp: UnixTimestamp,
        close_timestamp: UnixTimestamp,
        amount: u64,
        minimum_out: u64,
    },

    /// ACCOUNTS EXPECTED
    /// 1. [signer] - the signer of the transaction, annd the wallet address of the squad member
    /// 2. [writable] - the squad account
    /// 3. [] - the squad governance mint account
    /// 4. [] - the proposal account (PDA)
    /// 5. [] - the users governance PDA
    /// 6. [writable] - the vote record account
    /// 7. [] - the system program account
    /// 8. [] - the rent sysvar account
    /// 9. [] - the squads program account
    CastVote { vote: u8 },

    /// ACCOUNTS EXPECTED
    /// 1. [signer] - the signer of the transaction, annd the wallet address of the squad member
    /// 2. [writable?] - the squad account
    /// 3. [] - the squad governance mint account
    /// 4. [writable?] - the proposal account (PDA)
    /// 5. [writable?] - the execution source account
    /// 6. [] - the execution destination account
    /// 7. [] - the system program account
    /// 8. [] - the token program account
    /// 8. [] - the associated token program account
    /// 9. [] - the rent sysvar account
    ExecuteProposal { random_id: String },

    /// ACCOUNTS EXPECTED - DEPRECATED
    /// 1. [signer] - the signer of the transaction, annd the wallet address of the squad member
    /// 2. [writable?] - the squad account
    /// 7. [] - the system program account
    /// 8. [] - the token program account
    /// 8. [] - the associated token program account
    // QuitSquad,

    /// ACCOUNTS EXPECTED
    /// 1. [signer] - the signer of the transaction, annd the wallet address of the squad member
    /// 2. [writable] - the squad account
    /// 3. [] - the proposal account (PDA)
    /// 4. [writable] - the vote record account
    /// 5. [] - the system program account
    /// 6. [] - the rent sysvar account
    /// 7. [] - the squads program account
    CastMultisigVote { vote: u8 },

    /// ACCOUNTS EXPECTED
    /// 1. [signer] - the signer of the transaction, annd the wallet address of the squad member
    /// 2. [writable?] - the squad account
    /// 3. [writable?] - the proposal account (PDA)
    /// 4. [writable?] - the execution source account
    /// 5. [] - the execution destination account
    /// 6. [] - the system program account
    /// 7. [] - the token program account
    /// 8. [] - the associated token program account
    /// 9. [] - the rent sysvar account
    ExecuteMultisigProposal { random_id: String },
}

impl SquadInstruction {
    /// Unpacks a byte buffer into a [SquadInstruction](enum.SquadInstruction.html).
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        let (tag, rest) = input.split_first().ok_or(InvalidInstruction)?;

        Ok(match tag {
            0 => Self::CreateSquad {
                allocation_type: rest[0],
                vote_support: rest[1],
                vote_quorum: rest[2],
                core_threshold: rest[3],
                squad_name: Self::unpack_squad_name(&rest[4..28])?,
                description: Self::unpack_squad_description(&rest[28..64])?,
                token: Self::unpack_squad_token(&rest[64..70])?,
                random_id: Self::unpack_squad_random_id(&rest[70..80])?,
            },

            1 => Self::CreateMultisig {
                vote_quorum: rest[0],
                squad_name: Self::unpack_squad_name(&rest[1..25])?,
                description: Self::unpack_squad_description(&rest[25..61])?,
                random_id: Self::unpack_squad_random_id(&rest[61..71])?,
                members_num: rest[71],
            },

            // creates a new account for a proposal
            2 => Self::CreateProposalAccount {
                proposal_type: rest[0],
                title: Self::unpack_proposal_title(&rest[1..37])?,
                description: Self::unpack_proposal_description(&rest[37..533])?,
                link: Self::unpack_proposal_link(&rest[533..581])?,
                votes_num: rest[581],
                vote_labels: Self::unpack_proposal_labels(&rest[582..802])?,
                start_timestamp: Self::unpack_proposal_start(&rest[802..810])?,
                close_timestamp: Self::unpack_proposal_close(&rest[810..818])?,
                amount: Self::unpack_proposal_amount_in(rest)?,
                minimum_out: Self::unpack_proposal_amount_out(rest)?,
            },

            // Proposal vote (private squad)
            3 => Self::CastVote { vote: rest[0] },

            // Cast vote for multisig
            4 => Self::CastMultisigVote { vote: rest[0] },

            // execute the proposal
            5 => Self::ExecuteProposal {
                random_id: Self::unpack_wsol_random_id(rest)?,
            },

            // execute multisig proposal
            6 => Self::ExecuteMultisigProposal {
                random_id: Self::unpack_wsol_random_id(rest)?,
            },

            7 => Self::AddMembersToSquad {
                members_num: rest[0],
                allocation_table: Self::unpack_add_members_allocation_table(rest)?,
            },

            // Deprecated
            // 8 => Self::QuitSquad,
            _ => return Err(InvalidInstruction.into()),
        })
    }

    // SQUAD ACCOUNT INIT unpacks
    fn unpack_squad_name(input: &[u8]) -> Result<String, ProgramError> {
        let name = String::from_utf8(input.to_vec()).unwrap();
        Ok(name)
    }
    fn unpack_squad_description(input: &[u8]) -> Result<String, ProgramError> {
        let description = String::from_utf8(input.to_vec()).unwrap();
        Ok(description)
    }

    fn unpack_squad_token(input: &[u8]) -> Result<String, ProgramError> {
        let token = String::from_utf8(input.to_vec()).unwrap();
        Ok(token)
    }

    fn unpack_squad_random_id(input: &[u8]) -> Result<String, ProgramError> {
        let random_id = String::from_utf8(input.to_vec()).unwrap();
        Ok(random_id)
    }

    fn unpack_wsol_random_id(input: &[u8]) -> Result<String, ProgramError> {
        let mut string: String = String::from("0000000000000000");
        if input.len() >= 16 {
            string = String::from_utf8(input[0..16].try_into().unwrap()).unwrap();
        }
        Ok(string)
    }

    //
    // PROPOSAL INIT UNPACKS
    //
    fn unpack_proposal_title(input: &[u8]) -> Result<String, ProgramError> {
        // let title_raw = input.get(1..37).unwrap();
        let title = String::from_utf8(input.to_vec()).unwrap();
        Ok(title)
    }
    fn unpack_proposal_description(input: &[u8]) -> Result<String, ProgramError> {
        // let description_raw = input.get(37..533).unwrap();
        let description = String::from_utf8(input.to_vec()).unwrap();
        Ok(description)
    }
    fn unpack_proposal_link(input: &[u8]) -> Result<String, ProgramError> {
        // let link_raw = input.get(533..581).unwrap();
        let link = String::from_utf8(input.to_vec()).unwrap();
        Ok(link)
    }
    fn unpack_proposal_labels(input: &[u8]) -> Result<Vec<String>, ProgramError> {
        // let labels_raw = input.get(582..802).unwrap().to_vec();
        let labels_iter = input.chunks_exact(44);
        let labels: Vec<String> = labels_iter
            .map(|str_chunk| String::from_utf8(str_chunk.to_vec()).unwrap())
            .collect();
        Ok(labels)
    }
    fn unpack_proposal_start(input: &[u8]) -> Result<i64, ProgramError> {
        let start_timestamp_raw: [u8; 8] = input.try_into().unwrap();
        let start_timestamp = i64::from_le_bytes(start_timestamp_raw);
        Ok(start_timestamp)
    }
    fn unpack_proposal_close(input: &[u8]) -> Result<i64, ProgramError> {
        let close_timestamp_raw: [u8; 8] = input.try_into().unwrap();
        let close_timestamp = i64::from_le_bytes(close_timestamp_raw);
        Ok(close_timestamp)
    }
    fn unpack_proposal_amount_in(input: &[u8]) -> Result<u64, ProgramError> {
        let mut amount_in: [u8; 8] = [0; 8];
        if input.len() >= 826 {
            amount_in = input[818..826].try_into().unwrap();
        }
        Ok(u64::from_le_bytes(amount_in))
    }
    fn unpack_proposal_amount_out(input: &[u8]) -> Result<u64, ProgramError> {
        let mut amount_out: [u8; 8] = [0; 8];
        if input.len() >= 834 {
            amount_out = input[826..834].try_into().unwrap();
        }
        Ok(u64::from_le_bytes(amount_out))
    }

    fn unpack_add_members_allocation_table(input: &[u8]) -> Result<Vec<u64>, ProgramError> {
        let members_num = input[0];
        let slice_size = (members_num * 8) as usize;
        let slice = input.get(9..slice_size + 9).unwrap();
        let mut iter = slice.chunks_exact(8);
        let mut allocation_table = Vec::<u64>::new();
        for _i in 0..members_num {
            let alloc = iter
                .next()
                .and_then(|slice| slice.try_into().ok())
                .map(u64::from_le_bytes)
                .unwrap();
            allocation_table.push(alloc);
        }
        Ok(allocation_table)
    }
}

#[cfg(test)]
mod tests {
    // use super::*;

    #[test]
    fn squad_create_instruction() {}
}
