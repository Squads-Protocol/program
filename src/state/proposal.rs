use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
    pubkey::Pubkey,
};

use num_derive::FromPrimitive;
use std::convert::TryInto;

use crate::UnixTimestamp;

const PUBLIC_KEY_BYTES: usize = 32;
const TIMESTAMP_BYTES: usize = 8;

// proposal bytes
const PROPOSAL_SETTING_BYTES: usize = 1;
const PROPOSAL_EXECUTION_AMOUNT_BYTES: usize = 8;
const PROPOSAL_EXECUTION_SOURCE_BYTES: usize = PUBLIC_KEY_BYTES;
const PROPOSAL_EXECUTION_DESTINATION_BYTES: usize = PUBLIC_KEY_BYTES;
const PROPOSAL_TITLE_BYTES: usize = 36;
const PROPOSAL_DESCRIPTION_BYTES: usize = 496;
const PROPOSAL_LINK_BYTES: usize = 48;
const PROPOSAL_VOTE_OPTIONS_NUM: usize = 5;
const PROPOSAL_HAS_VOTED_NUM_BYTES: usize = 1;
const PROPOSAL_HAS_VOTED_BYTES: usize = (150 * PUBLIC_KEY_BYTES) + 4;
const PROPOSAL_OPTIONS_BYTES: usize = PROPOSAL_VOTE_OPTIONS_NUM * 8;
const PROPOSAL_OPTIONS_LABELS_BYTES: usize = PROPOSAL_VOTE_OPTIONS_NUM * 44;
const PROPOSAL_RESERVED_BYTES: usize = 8 * 16;
const SUPPLY_AT_EXECUTE_BYTES: usize = 8;
const MEMBERS_AT_EXECUTE_BYTES: usize = 1;
const THRESHOLD_AT_EXECUTE_BYTES: usize = 1;
const PROPOSAL_INDEX_BYTES: usize = 4;

#[derive(FromPrimitive)]
pub enum ProposalType {
    Text = 0,
    Support = 1,
    Quorum = 2,
    WithdrawSol = 3,
    WithdrawSpl = 4,
    AddMember = 5,
    RemoveMember = 6,
    MintMemberToken = 7,
    Swap = 8,
}

// PROPOSAL STRUCT
const PROPOSAL_TOTAL_BYTES: usize = PROPOSAL_SETTING_BYTES +                // is_initialized 1
    PROPOSAL_SETTING_BYTES +                // proposal_type 1
    PROPOSAL_EXECUTION_AMOUNT_BYTES +       // execution_amount 8
    PROPOSAL_EXECUTION_AMOUNT_BYTES +       // execution_amount_out 8
    PROPOSAL_EXECUTION_SOURCE_BYTES +       // execution_source 32
    PROPOSAL_EXECUTION_DESTINATION_BYTES +  // execution_source 32 
    PUBLIC_KEY_BYTES +                      // creator 32
    PUBLIC_KEY_BYTES +                      // squad_address 32
    PROPOSAL_TITLE_BYTES +                  // title of the proposal 36
    PROPOSAL_DESCRIPTION_BYTES +            // description of the proposal 496
    PROPOSAL_LINK_BYTES +                   // link bytes 48
    PROPOSAL_SETTING_BYTES +                // vote options num 1
    PROPOSAL_HAS_VOTED_NUM_BYTES +
    PROPOSAL_HAS_VOTED_BYTES +              // 100 * 32 + 4
    PROPOSAL_OPTIONS_BYTES +                // 5 * 4
    PROPOSAL_OPTIONS_LABELS_BYTES +         // 5 * 44 = 220
    TIMESTAMP_BYTES +                       // start date 8
    TIMESTAMP_BYTES +                       // proposal 8
    TIMESTAMP_BYTES +                       // created_on bytes 8
    SUPPLY_AT_EXECUTE_BYTES +               // supply_at_execute 8
    MEMBERS_AT_EXECUTE_BYTES +              // members_at_execute 1
    THRESHOLD_AT_EXECUTE_BYTES +              // members_at_execute 1
    PROPOSAL_SETTING_BYTES +                // executed 1
    PROPOSAL_SETTING_BYTES +                // execute_ready 1
    TIMESTAMP_BYTES +                       // execution_date bytes 8
    PROPOSAL_SETTING_BYTES +                // instruction_index 1
    PROPOSAL_SETTING_BYTES +                // multiple_choice 1
    PUBLIC_KEY_BYTES +                      // executed_by 32
    PROPOSAL_INDEX_BYTES +                  // the proposal index
    PROPOSAL_RESERVED_BYTES; // reserved for updates

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
pub struct Proposal {
    pub is_initialized: bool,
    // 0 - text proposal
    // 1 - change support
    // 2 - change quorum
    // 3 - emergency quorum
    // 4 - withdraw SOL
    // 5 - withdraw token
    // 6 - add member
    // 7 - remove member
    // 8 - mint more tokens to a member
    pub proposal_type: u8,
    pub execution_amount: u64,
    pub execution_amount_out: u64,
    pub execution_source: Pubkey,
    pub execution_destination: Pubkey,
    pub creator: Pubkey,
    pub squad_address: Pubkey,
    pub title: String,
    pub description: String,
    pub link: String,
    // number of vote options
    pub votes_num: u8,
    pub has_voted_num: u8,
    pub has_voted: Vec<Pubkey>,
    pub votes: Vec<u64>,
    // labels of the vote options
    pub votes_labels: Vec<String>,
    pub start_timestamp: UnixTimestamp,
    pub close_timestamp: UnixTimestamp,
    pub created_timestamp: UnixTimestamp,
    pub supply_at_execute: u64,
    pub members_at_execute: u8,
    pub threshold_at_execute: u8,
    pub executed: bool,
    pub execute_ready: bool,
    pub execution_date: UnixTimestamp,

    pub instruction_index: u8,
    pub multiple_choice: bool,

    pub executed_by: Pubkey,
    pub proposal_index: u32,
    // reserved for future updates
    pub reserved: [u64; 16],
}

impl Sealed for Proposal {}

impl IsInitialized for Proposal {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

impl Proposal {
    pub fn save_text(
        &mut self,
        proposal_type: u8,
        title: String,
        description: String,
        link: String,
        initializer: &Pubkey,
        votes_num: u8,
        squad_account_key: &Pubkey,
        vote_labels: Vec<String>,
        start_timestamp: i64,
        close_timestamp: i64,
        created_timestamp: i64,
        proposal_index: u32,
    ) {
        self.is_initialized = true;
        self.proposal_type = proposal_type;
        self.title = title;
        self.description = description;
        self.link = link;
        self.creator = *initializer;
        self.votes_num = votes_num;
        self.squad_address = *squad_account_key;
        self.votes_labels = vote_labels;
        self.start_timestamp = start_timestamp;
        self.close_timestamp = close_timestamp;
        self.created_timestamp = created_timestamp;
        self.executed = false;
        self.execution_amount = 0;
        self.execute_ready = false;
        self.execution_date = 0 as i64;
        self.proposal_index = proposal_index;
    }

    pub fn save_core(
        &mut self,
        proposal_type: u8,
        title: String,
        description: String,
        link: String,
        initializer: &Pubkey,
        votes_num: u8,
        squad_account_key: &Pubkey,
        vote_labels: Vec<String>,
        start_timestamp: i64,
        close_timestamp: i64,
        created_timestamp: i64,
        amount: u64,
        proposal_index: u32,
    ) {
        self.is_initialized = true;
        self.proposal_type = proposal_type;
        self.title = title;
        self.description = description;
        self.link = link;
        self.creator = *initializer;
        self.votes_num = votes_num;
        self.squad_address = *squad_account_key;
        self.votes_labels = vote_labels;
        self.start_timestamp = start_timestamp;
        self.close_timestamp = close_timestamp;
        self.execution_amount = amount;
        self.created_timestamp = created_timestamp;
        self.executed = false;
        self.execute_ready = false;
        self.execution_date = 0 as i64;
        self.proposal_index = proposal_index;
    }

    pub fn save_withdraw(
        &mut self,
        proposal_type: u8,
        title: String,
        description: String,
        link: String,
        source: &Pubkey,
        destination: &Pubkey,
        initializer: &Pubkey,
        votes_num: u8,
        squad_account_key: &Pubkey,
        vote_labels: Vec<String>,
        start_timestamp: i64,
        close_timestamp: i64,
        created_timestamp: i64,
        amount: u64,
        proposal_index: u32,
    ) {
        self.is_initialized = true;
        self.proposal_type = proposal_type;
        self.title = title;
        self.description = description;
        self.link = link;
        self.execution_source = *source;
        self.execution_destination = *destination;
        self.creator = *initializer;
        self.votes_num = votes_num;
        self.squad_address = *squad_account_key;
        self.votes_labels = vote_labels;
        self.start_timestamp = start_timestamp;
        self.close_timestamp = close_timestamp;
        self.execution_amount = amount;
        self.created_timestamp = created_timestamp;
        self.executed = false;
        self.execute_ready = false;
        self.execution_date = 0 as i64;
        self.proposal_index = proposal_index;
    }

    pub fn save_member(
        &mut self,
        proposal_type: u8,
        title: String,
        description: String,
        link: String,
        member: &Pubkey,
        initializer: &Pubkey,
        votes_num: u8,
        squad_account: &Pubkey,
        vote_labels: Vec<String>,
        start_timestamp: i64,
        close_timestamp: i64,
        created_timestamp: i64,
        amount: u64,
        proposal_index: u32,
    ) {
        self.is_initialized = true;
        self.proposal_type = proposal_type;
        self.title = title;
        self.description = description;
        self.link = link;
        self.execution_source = *squad_account;
        self.execution_destination = *member;
        self.creator = *initializer;
        self.votes_num = votes_num;
        self.squad_address = *squad_account;
        self.votes_labels = vote_labels;
        self.start_timestamp = start_timestamp;
        self.close_timestamp = close_timestamp;
        self.created_timestamp = created_timestamp;
        self.executed = false;
        self.execution_amount = amount;
        self.execute_ready = false;
        self.execution_date = 0 as i64;
        self.proposal_index = proposal_index;
    }

    pub fn save_swap(
        &mut self,
        proposal_type: u8,
        title: String,
        description: String,
        link: String,
        source: &Pubkey,
        destination: &Pubkey,
        initializer: &Pubkey,
        votes_num: u8,
        squad_account: &Pubkey,
        vote_labels: Vec<String>,
        start_timestamp: i64,
        close_timestamp: i64,
        created_timestamp: i64,
        amount: u64,
        minimum_out: u64,
        proposal_index: u32,
    ) {
        self.is_initialized = true;
        self.proposal_type = proposal_type;
        self.title = title;
        self.description = description;
        self.link = link;
        self.execution_source = *source;
        self.execution_destination = *destination;
        self.creator = *initializer;
        self.votes_num = votes_num;
        self.squad_address = *squad_account;
        self.votes_labels = vote_labels;
        self.start_timestamp = start_timestamp;
        self.close_timestamp = close_timestamp;
        self.execution_amount = amount;
        self.execution_amount_out = minimum_out;
        self.created_timestamp = created_timestamp;
        self.executed = false;
        self.execute_ready = false;
        self.execution_date = 0 as i64;
        self.proposal_index = proposal_index;
    }
}

impl Pack for Proposal {
    const LEN: usize = PROPOSAL_TOTAL_BYTES;

    fn pack_into_slice(&self, dst: &mut [u8]) {
        let dst = array_mut_ref![dst, 0, PROPOSAL_TOTAL_BYTES];

        let (
            is_initialized_dst,
            proposal_type_dst,
            execution_amount_dst,
            execution_amount_out_dst,
            execution_source_dst,
            execution_destination_dst,
            creator_dst,
            squad_address_dst,
            title_dst,
            description_dst,
            link_dst,
            // will be fixed to 5 options, max
            votes_num_dst,
            has_voted_num_dst,
            has_voted_dst,
            votes_dst,
            votes_labels_dst,
            // timestamp bytes
            start_timestamp_dst,
            close_timestamp_dst,
            created_timestamp_dst,
            supply_at_execute_dst,
            members_at_execute_dst,
            threshold_at_execute_dst,
            executed_dst,
            execute_ready_dst,
            execution_date_dst,
            instruction_index_dst,
            multiple_choice_dst,
            executed_by_dst,
            proposal_index_dst,
            _reserved,
        ) = mut_array_refs![
            dst,
            PROPOSAL_SETTING_BYTES,               // is_initialized 1
            PROPOSAL_SETTING_BYTES,               // type 1
            PROPOSAL_EXECUTION_AMOUNT_BYTES,      // execution amount 8
            PROPOSAL_EXECUTION_AMOUNT_BYTES,      // execution amount out 8
            PROPOSAL_EXECUTION_SOURCE_BYTES,      // execution source 32
            PROPOSAL_EXECUTION_DESTINATION_BYTES, // execution destination 32
            PUBLIC_KEY_BYTES,                     // creator 32
            PUBLIC_KEY_BYTES,                     // squad address 32
            PROPOSAL_TITLE_BYTES,                 // title of the proposal 36
            PROPOSAL_DESCRIPTION_BYTES,           // description of the proposal 496
            PROPOSAL_LINK_BYTES,                  // link bytes 48
            PROPOSAL_SETTING_BYTES,               // vote options num 1
            PROPOSAL_HAS_VOTED_NUM_BYTES,         // 1
            PROPOSAL_HAS_VOTED_BYTES,             // 100 * 32
            PROPOSAL_OPTIONS_BYTES,               // 5 * 4
            PROPOSAL_OPTIONS_LABELS_BYTES,        // 5 * 44 = 220
            TIMESTAMP_BYTES,                      // start date 8
            TIMESTAMP_BYTES,                      // proposal 8
            TIMESTAMP_BYTES,                      // created_on bytes 8
            SUPPLY_AT_EXECUTE_BYTES,              // supply_at_execute 8
            MEMBERS_AT_EXECUTE_BYTES,             // members_at_execute 1
            THRESHOLD_AT_EXECUTE_BYTES,           // threshold_at_execute 1
            PROPOSAL_SETTING_BYTES,               // executed 1
            PROPOSAL_SETTING_BYTES,               // execute_ready 1
            TIMESTAMP_BYTES,                      // execution_date bytes 8
            PROPOSAL_SETTING_BYTES,               // instruction_index 1
            PROPOSAL_SETTING_BYTES,               // multiple_choice 1
            PUBLIC_KEY_BYTES,                     // executed_by 32
            PROPOSAL_INDEX_BYTES,                 // proposal index
            PROPOSAL_RESERVED_BYTES
        ];

        let Proposal {
            is_initialized,
            proposal_type,
            execution_amount,
            execution_amount_out,
            execution_source,
            execution_destination,
            creator,
            squad_address,
            title,
            description,
            link,
            // will be fixed to 5 options, max
            votes_num,
            has_voted_num,
            has_voted,
            votes,
            votes_labels,
            start_timestamp,
            close_timestamp,
            created_timestamp,
            supply_at_execute,
            members_at_execute,
            threshold_at_execute,
            executed,
            execute_ready,
            execution_date,
            instruction_index,
            multiple_choice,
            executed_by,
            proposal_index,
            reserved: _,
        } = self;

        is_initialized_dst[0] = *is_initialized as u8;
        *proposal_type_dst = proposal_type.to_le_bytes();
        *execution_amount_dst = execution_amount.to_le_bytes();
        *execution_amount_out_dst = execution_amount_out.to_le_bytes();
        execution_source_dst.copy_from_slice(execution_source.as_ref());
        execution_destination_dst.copy_from_slice(execution_destination.as_ref());
        creator_dst.copy_from_slice(creator.as_ref());
        squad_address_dst.copy_from_slice(squad_address.as_ref());
        // pack the description

        let title_ser = title.as_bytes();
        title_dst[..title_ser.len()].copy_from_slice(title_ser);

        let description_ser = description.as_bytes();
        description_dst[..description_ser.len()].copy_from_slice(description_ser);

        let link_ser = link.as_bytes();
        link_dst[..link_ser.len()].copy_from_slice(link_ser);

        *votes_num_dst = votes_num.to_le_bytes();

        let votes_len = votes.len();
        let mut votes_check = votes.clone();
        for _i in 0..PROPOSAL_VOTE_OPTIONS_NUM - votes_len {
            votes_check.push(0);
        }

        let vote_byte_collect: Vec<Vec<u8>> = votes_check
            .iter()
            .map(|v| v.to_le_bytes().to_vec())
            .collect();

        let votes_ser: Vec<u8> = vote_byte_collect.into_iter().flatten().collect();

        *has_voted_num_dst = has_voted_num.to_le_bytes();

        let has_voted_ser_num = has_voted.len();
        let has_voted_ser: Vec<u8> = has_voted.try_to_vec().unwrap();
        has_voted_dst[..has_voted_ser_num * 32 as usize + 4 as usize]
            .copy_from_slice(&has_voted_ser);

        votes_dst[..].copy_from_slice(&votes_ser.as_slice());
        *created_timestamp_dst = created_timestamp.to_le_bytes();
        *start_timestamp_dst = start_timestamp.to_le_bytes();
        *close_timestamp_dst = close_timestamp.to_le_bytes();

        *supply_at_execute_dst = supply_at_execute.to_le_bytes();
        *members_at_execute_dst = members_at_execute.to_le_bytes();
        *threshold_at_execute_dst = threshold_at_execute.to_le_bytes();

        executed_dst[0] = *executed as u8;
        execute_ready_dst[0] = *execute_ready as u8;
        *execution_date_dst = execution_date.to_le_bytes();

        *instruction_index_dst = instruction_index.to_le_bytes();
        multiple_choice_dst[0] = *multiple_choice as u8;

        executed_by_dst.copy_from_slice(executed_by.as_ref());

        let votes_labels_len = votes_labels.len();
        let mut votes_labels_check = votes_labels.clone();

        // make sure that we fill up all labels
        for _s in 0..PROPOSAL_VOTE_OPTIONS_NUM - votes_labels_len {
            votes_labels_check.push(String::from_utf8(vec![0; 44]).unwrap());
        }

        // make sure all labels are 44 len
        let votes_labels_ser: Vec<u8> = votes_labels_check
            .iter()
            .map(|l| {
                let mut s_check = l.clone();
                for _c in 0..(44 - s_check.len()) {
                    s_check.push('\u{0}');
                }
                s_check.as_bytes().to_vec()
            })
            .flatten()
            .collect();

        votes_labels_dst[..].copy_from_slice(votes_labels_ser.as_slice());

        *proposal_index_dst = proposal_index.to_le_bytes();
    }

    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let src = array_ref![src, 0, PROPOSAL_TOTAL_BYTES];
        let (
            is_initialized,
            proposal_type,
            execution_amount,
            execution_amount_out,
            execution_source,
            execution_destination,
            creator,
            squad_address,
            title_src,
            description_src,
            link_src,
            // will be fixed to 16 options, max
            votes_num,
            has_voted_num,
            has_voted_src,
            votes,
            votes_labels_src,
            start_timestamp,
            close_timestamp,
            created_timestamp,
            supply_at_execute,
            members_at_execute,
            threshold_at_execute,
            executed,
            execute_ready,
            execution_date,
            instruction_index,
            multiple_choice,
            executed_by,
            proposal_index,
            _reserved,
        ) = array_refs![
            src,
            PROPOSAL_SETTING_BYTES,          // is_initialized
            PROPOSAL_SETTING_BYTES,          // proposal_type
            PROPOSAL_EXECUTION_AMOUNT_BYTES, // execution amount
            PROPOSAL_EXECUTION_AMOUNT_BYTES, // execution amount out
            PROPOSAL_EXECUTION_SOURCE_BYTES, // execution source
            PROPOSAL_EXECUTION_DESTINATION_BYTES,
            PUBLIC_KEY_BYTES,             // proposal creator
            PUBLIC_KEY_BYTES,             // squad_account
            PROPOSAL_TITLE_BYTES,         // title
            PROPOSAL_DESCRIPTION_BYTES,   // description
            PROPOSAL_LINK_BYTES,          // link
            PROPOSAL_SETTING_BYTES,       // numer of vote options
            PROPOSAL_HAS_VOTED_NUM_BYTES, // bytes for has voted num
            PROPOSAL_HAS_VOTED_BYTES,     // bytes for Vec Pubkey
            PROPOSAL_OPTIONS_BYTES,       // bytes for BTreeMap buckets
            PROPOSAL_OPTIONS_LABELS_BYTES,
            TIMESTAMP_BYTES,            // start proposal
            TIMESTAMP_BYTES,            // close proposal
            TIMESTAMP_BYTES,            // created on
            SUPPLY_AT_EXECUTE_BYTES,    // supply_at_execute 8
            MEMBERS_AT_EXECUTE_BYTES,   // members_at_execute 1
            THRESHOLD_AT_EXECUTE_BYTES, // threshold_at_execute 1
            PROPOSAL_SETTING_BYTES,     // executed 1
            PROPOSAL_SETTING_BYTES,     // execute_ready 1
            TIMESTAMP_BYTES,            // execution_date
            PROPOSAL_SETTING_BYTES,     // instruction_index 1
            PROPOSAL_SETTING_BYTES,     // multiple_choice 1
            PUBLIC_KEY_BYTES,           // executed_by 32
            PROPOSAL_INDEX_BYTES,       // proposal index
            PROPOSAL_RESERVED_BYTES
        ];

        let is_initialized = match is_initialized {
            [0] => false,
            [1] => true,
            _ => return Err(ProgramError::InvalidAccountData),
        };

        let executed = match executed {
            [0] => false,
            [1] => true,
            _ => return Err(ProgramError::InvalidAccountData),
        };

        let execute_ready = match execute_ready {
            [0] => false,
            [1] => true,
            _ => return Err(ProgramError::InvalidAccountData),
        };

        let multiple_choice = match multiple_choice {
            [0] => false,
            [1] => true,
            _ => return Err(ProgramError::InvalidAccountData),
        };

        let title_deser = String::from_utf8(title_src.to_vec()).unwrap();
        let description_deser = String::from_utf8(description_src.to_vec()).unwrap();
        let link_deser = String::from_utf8(link_src.to_vec()).unwrap();
        let votes_num_deser = votes_num[0];

        let votes_iter = votes.chunks(8);
        let votes = votes_iter
            .map(|slice| u64::from_le_bytes(slice.try_into().unwrap()))
            .collect();

        let mut has_voted_deser = Vec::<Pubkey>::new();
        let has_voted_num = u8::from_le_bytes(*has_voted_num);
        if has_voted_num > 0 {
            has_voted_deser = Vec::<Pubkey>::try_from_slice(
                &has_voted_src[0..32 * has_voted_num as usize + 4 as usize],
            )
            .unwrap();
        }

        let vote_options_deser: Vec<String> = votes_labels_src
            .chunks_exact(44)
            .map(|oc| String::from_utf8(oc.to_vec()).unwrap())
            .collect();

        Ok(Proposal {
            // low level settings
            is_initialized,
            // squad settings
            proposal_type: u8::from_le_bytes(*proposal_type),
            execution_amount: u64::from_le_bytes(*execution_amount),
            execution_amount_out: u64::from_le_bytes(*execution_amount_out),
            execution_source: Pubkey::new(execution_source),
            execution_destination: Pubkey::new(execution_destination),
            creator: Pubkey::new(creator),
            squad_address: Pubkey::new(squad_address),
            title: title_deser,
            description: description_deser,
            link: link_deser,
            votes_num: votes_num_deser,
            has_voted_num,
            has_voted: has_voted_deser,
            votes,
            votes_labels: vote_options_deser,
            // proposal nonce
            start_timestamp: i64::from_le_bytes(*start_timestamp),
            close_timestamp: i64::from_le_bytes(*close_timestamp),
            //creation time
            created_timestamp: i64::from_le_bytes(*created_timestamp),
            // execution params
            supply_at_execute: u64::from_le_bytes(*supply_at_execute),
            members_at_execute: u8::from_le_bytes(*members_at_execute),
            threshold_at_execute: u8::from_le_bytes(*threshold_at_execute),
            executed,
            execute_ready,
            execution_date: i64::from_le_bytes(*execution_date),
            instruction_index: u8::from_le_bytes(*instruction_index),
            multiple_choice,
            executed_by: Pubkey::new(executed_by),
            proposal_index: u32::from_le_bytes(*proposal_index),
            reserved: [0; 16],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono;
    use solana_program::pubkey::Pubkey;

    #[test]
    fn proposal_struct_size() {
        let mut test_dst: [u8; PROPOSAL_TOTAL_BYTES] = [0; PROPOSAL_TOTAL_BYTES];
        let description_vec = vec![0; 496];
        let description = String::from_utf8(description_vec).unwrap();

        let link_vec = vec![0; 48];
        let link = String::from_utf8(link_vec).unwrap();

        let votes = vec![0, 3, 2000, 7, 0];

        let has_voted = vec![
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
        ];
        let has_voted_num = has_voted.len() as u8;

        let votes_labels_vec = vec![0; 5];
        let votes_labels = votes_labels_vec
            .iter()
            .map(|v| String::from("asdfasdfasdfasdfasdfasdfasdfasdfasdfasdfadfa"))
            .collect();

        let test_proposal = Proposal {
            is_initialized: true,
            proposal_type: 0,
            execution_amount: 100,
            execution_amount_out: 100,
            execution_source: Pubkey::new_unique(),
            execution_destination: Pubkey::new_unique(),
            creator: Pubkey::new_unique(),
            squad_address: Pubkey::new_unique(),
            title: String::from("This is a test adfasdfasdfasdfasdfff"),
            description,
            link, // 160 - fixed bytes
            // will be fixed to 16 options, max
            votes_num: 5,
            has_voted_num,
            has_voted,
            votes,
            // will be fixed to 16 items to match above
            votes_labels,
            start_timestamp: chrono::offset::Utc::now().timestamp(),
            close_timestamp: chrono::offset::Utc::now().timestamp(),
            created_timestamp: chrono::offset::Utc::now().timestamp(),
            supply_at_execute: 0,
            members_at_execute: 0,
            threshold_at_execute: 0,
            executed: false,
            execute_ready: false,
            execution_date: chrono::offset::Utc::now().timestamp(),
            instruction_index: 0,
            multiple_choice: false,
            executed_by: Pubkey::new_unique(),
            proposal_index: 0,
            reserved: [0; 16],
        };

        Proposal::pack(test_proposal, &mut test_dst);

        let mut test_proposal_deser = Proposal::unpack_unchecked(&test_dst).unwrap();
        println!("proposal unpack: {:?}", test_proposal_deser);

        test_proposal_deser.has_voted.push(Pubkey::new_unique());
        test_proposal_deser.has_voted_num = test_proposal_deser.has_voted.len() as u8;
        Proposal::pack(test_proposal_deser, &mut test_dst);

        let test_proposal_deser = Proposal::unpack_unchecked(&test_dst).unwrap();
        println!("proposal unpack: {:?}", test_proposal_deser);

        println!("proposal packed len: {:?}", Proposal::get_packed_len());
        println!("total proposal size: {:?}", PROPOSAL_TOTAL_BYTES);
    }
}
