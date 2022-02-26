use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use borsh::{BorshDeserialize, BorshSerialize};
use num_derive::FromPrimitive;
use solana_program::{
    clock::Clock,
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use std::collections::BTreeMap;

use crate::count_from_le;
use crate::transform_u32_to_array_of_u8;

#[derive(FromPrimitive)]
pub enum AllocationType {
    TeamCoordination = 1,
    Multisig = 2,
}

// Squad Bytes
const SQUAD_MAX_MEMBERS: usize = 150;
const SQUAD_SETTING_BYTES: usize = 1;
const SQUAD_NAME_BYTES: usize = 24;
const SQUAD_DESCRIPTION_BYTES: usize = 36;
const SQUAD_TOKEN_BYTES: usize = 6;
const PUBLIC_KEY_BYTES: usize = 32;
const PROPOSAL_NONCE_BYTES: usize = 4;
const MEMBER_LENGTH_BYTES: usize = 4;
const TIMESTAMP_BYTES: usize = 8;
const SQUAD_RESERVED_BYTES: usize = 8 * 32;
const SQUAD_RANDOM_ID_BYTES: usize = 10;
const CHILD_INDEX_BYTES: usize = 4;
const MEMBER_LOCK_BYTES: usize = 4;

// SQUAD STRUCT
const SQUAD_TOTAL_BYTES: usize = SQUAD_SETTING_BYTES +  // is_initialized
    SQUAD_SETTING_BYTES +       // open
    SQUAD_SETTING_BYTES +       // emergency_lock
    SQUAD_SETTING_BYTES +       // allocation_type
    SQUAD_SETTING_BYTES +       // vote_support
    SQUAD_SETTING_BYTES +       // vote_quorum
    SQUAD_SETTING_BYTES +       // core_threshold
    SQUAD_NAME_BYTES +          // bytes for the name
    SQUAD_DESCRIPTION_BYTES +
    SQUAD_TOKEN_BYTES +
    SQUAD_SETTING_BYTES +       // future_setting_1
    SQUAD_SETTING_BYTES +       // future_setting_2
    SQUAD_SETTING_BYTES +       // future_setting_3
    SQUAD_SETTING_BYTES +       // future_setting_4
    SQUAD_SETTING_BYTES +       // future_setting_5
    PUBLIC_KEY_BYTES +          // admin
    PUBLIC_KEY_BYTES +          // mint pda
    PUBLIC_KEY_BYTES +          // sol pda
    PUBLIC_KEY_BYTES +          // future_address 1
    PUBLIC_KEY_BYTES +          // future_address 2
    PUBLIC_KEY_BYTES +          // future_address 3
    PUBLIC_KEY_BYTES +          // future_address 4
    PUBLIC_KEY_BYTES +          // future_address 5
    PROPOSAL_NONCE_BYTES +      // proposal
    TIMESTAMP_BYTES +           // created on
    MEMBER_LENGTH_BYTES +       // bytes for the length num
    ((PUBLIC_KEY_BYTES * 2) * SQUAD_MAX_MEMBERS) + 4 +  // MEMBER STRUCTS
    SQUAD_RANDOM_ID_BYTES +     // random_id 10
    CHILD_INDEX_BYTES +         // child_index 4
    MEMBER_LOCK_BYTES +       // member lock bytes
    SQUAD_RESERVED_BYTES;

/// Member struct for a Squad, used in the members BTreeMap
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
pub struct Member {
    pub equity_token_account: Pubkey, // contributions_account: [u8; 32], // need to expand for each mint
}

/// The Squad Account struct
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
pub struct Squad {
    /// whether or not the squad has been initialized
    pub is_initialized: bool,

    /// whether or not the owner can still make changes (draft mode)
    pub open: bool,
    pub emergency_lock: bool,

    /// typical settings
    pub allocation_type: u8,
    pub vote_support: u8,
    pub vote_quorum: u8,
    pub core_threshold: u8,
    pub squad_name: String,
    pub description: String,
    pub token: String,

    // future settings placeholders
    pub future_setting_1: u8,
    pub future_setting_2: u8,
    pub future_setting_3: u8,
    pub future_setting_4: u8,
    pub future_setting_5: u8,

    /// misc address for squad specific settings
    // admin address for draft mode (open=true) only
    pub admin: Pubkey,
    pub sol_account: Pubkey,
    pub mint_address: Pubkey,

    pub future_address1: Pubkey,
    pub future_address2: Pubkey,
    pub future_address3: Pubkey,
    pub future_address4: Pubkey,
    pub future_address5: Pubkey,

    pub proposal_nonce: u32,
    pub created_on: i64,
    /// the squad member list
    pub members: BTreeMap<Pubkey, Member>,

    pub random_id: String,

    pub child_index: u32,
    pub member_lock_index: u32,
    // reserved for future updates
    pub reserved: [u64; 32],
}

impl Sealed for Squad {}

impl IsInitialized for Squad {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

impl Squad {
    pub fn add_member(&mut self, key: Pubkey, value: Member) {
        self.members.insert(key, value);
    }

    pub fn remove_member(&mut self, key: &Pubkey) {
        self.members.remove(key);
    }

    pub fn member_exists(&self, key: &Pubkey) -> bool {
        self.members.contains_key(key)
    }

    pub fn setup_tc(
        &mut self,
        allocation_type: u8,
        vote_support: u8,
        vote_quorum: u8,
        core_threshold: u8,
        squad_name: String,
        description: String,
        token: String,
        initializer: &Pubkey,
        mint_owner: &Pubkey,
        sol_account_owner_pda: &Pubkey,
        random_id: String,
    ) {
        self.is_initialized = true;
        self.open = true;
        self.emergency_lock = false;
        self.allocation_type = allocation_type;
        self.vote_support = vote_support;
        self.vote_quorum = vote_quorum;
        self.core_threshold = core_threshold;
        self.squad_name = squad_name;
        self.description = description;
        self.token = token;
        self.admin = *initializer;
        self.mint_address = *mint_owner;
        self.sol_account = *sol_account_owner_pda;
        self.created_on = Clock::get().unwrap().unix_timestamp;
        self.random_id = random_id;
    }

    pub fn setup_ms(
        &mut self,
        vote_quorum: u8,
        squad_name: String,
        description: String,
        initializer: &Pubkey,
        sol_account_owner_pda: &Pubkey,
        random_id: String,
    ) {
        self.is_initialized = true;
        self.open = false;
        self.emergency_lock = false;
        self.allocation_type = AllocationType::Multisig as u8;
        self.vote_support = 0;
        self.vote_quorum = vote_quorum;
        self.squad_name = squad_name;
        self.description = description;
        self.admin = *initializer;
        self.sol_account = *sol_account_owner_pda;
        self.created_on = Clock::get().unwrap().unix_timestamp;
        self.random_id = random_id;
    }
}

impl Pack for Squad {
    const LEN: usize = SQUAD_TOTAL_BYTES;
    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let src = array_ref![src, 0, SQUAD_TOTAL_BYTES];
        let (
            is_initialized,
            open,
            emergency_lock,
            // typical settings
            allocation_type,
            vote_support,
            vote_quorum,
            core_threshold,
            squad_name_src,
            description_src,
            token_src,
            // future settings placeholders
            future_setting_1,
            future_setting_2,
            future_setting_3,
            future_setting_4,
            future_setting_5,
            // misc address for squad specific settings
            // admin address for draft mode (open=true) only
            admin,
            mint_address,
            sol_account,
            future_address1,
            future_address2,
            future_address3,
            future_address4,
            future_address5,
            proposal_nonce,
            created_on,
            members_len,
            members_src,
            random_id,
            _child_index,
            member_lock_index,
            _reserved,
        ) = array_refs![
            src,
            SQUAD_SETTING_BYTES, // is_initialized
            SQUAD_SETTING_BYTES, // open
            SQUAD_SETTING_BYTES, // emergency_lock
            SQUAD_SETTING_BYTES, // allocation_type
            SQUAD_SETTING_BYTES, // vote_support
            SQUAD_SETTING_BYTES, // vote_quorum
            SQUAD_SETTING_BYTES, // core_threshold
            SQUAD_NAME_BYTES,    // bytes for the name
            SQUAD_DESCRIPTION_BYTES,
            SQUAD_TOKEN_BYTES,
            SQUAD_SETTING_BYTES,                              // future_setting_1
            SQUAD_SETTING_BYTES,                              // future_setting_2
            SQUAD_SETTING_BYTES,                              // future_setting_3
            SQUAD_SETTING_BYTES,                              // future_setting_4
            SQUAD_SETTING_BYTES,                              // future_setting_5
            PUBLIC_KEY_BYTES,                                 // admin
            PUBLIC_KEY_BYTES,                                 // mint pda
            PUBLIC_KEY_BYTES,                                 // sol pda
            PUBLIC_KEY_BYTES,                                 // future_address 1
            PUBLIC_KEY_BYTES,                                 // future_address 2
            PUBLIC_KEY_BYTES,                                 // future_address 3
            PUBLIC_KEY_BYTES,                                 // future_address 4
            PUBLIC_KEY_BYTES,                                 // future_address 5
            PROPOSAL_NONCE_BYTES,                             // proposal
            TIMESTAMP_BYTES,                                  // created on
            MEMBER_LENGTH_BYTES,                              // bytes for the length num
            ((PUBLIC_KEY_BYTES * 2) * SQUAD_MAX_MEMBERS) + 4, // Member structs
            SQUAD_RANDOM_ID_BYTES,
            CHILD_INDEX_BYTES,
            MEMBER_LOCK_BYTES,    // Member lock index
            SQUAD_RESERVED_BYTES  // reserved for future
        ];

        let is_initialized = match is_initialized {
            [0] => false,
            [1] => true,
            _ => return Err(ProgramError::InvalidAccountData),
        };

        let open = match open {
            [0] => false,
            [1] => true,
            _ => return Err(ProgramError::InvalidAccountData),
        };

        let emergency_lock = match emergency_lock {
            [0] => false,
            [1] => true,
            _ => return Err(ProgramError::InvalidAccountData),
        };

        let mut member_dser = BTreeMap::<Pubkey, Member>::new();
        let member_length = count_from_le(members_len);
        if member_length > 0 {
            member_dser =
                BTreeMap::<Pubkey, Member>::try_from_slice(&members_src[0..member_length]).unwrap()
        }

        // deserialize the string
        let squad_name_deser = String::from_utf8(squad_name_src.to_vec()).unwrap();
        let description_deser = String::from_utf8(description_src.to_vec()).unwrap();
        let token_deser = String::from_utf8(token_src.to_vec()).unwrap();
        let random_id_deser = String::from_utf8(random_id.to_vec()).unwrap();

        Ok(Squad {
            // low level settings
            is_initialized,
            open,
            emergency_lock,
            // squad settings
            allocation_type: u8::from_le_bytes(*allocation_type),
            vote_support: u8::from_le_bytes(*vote_support),
            vote_quorum: u8::from_le_bytes(*vote_quorum),
            core_threshold: u8::from_le_bytes(*core_threshold),
            squad_name: squad_name_deser,
            description: description_deser,
            token: token_deser,

            // reserved
            future_setting_1: u8::from_le_bytes(*future_setting_1),
            future_setting_2: u8::from_le_bytes(*future_setting_2),
            future_setting_3: u8::from_le_bytes(*future_setting_3),
            future_setting_4: u8::from_le_bytes(*future_setting_4),
            future_setting_5: u8::from_le_bytes(*future_setting_5),

            admin: Pubkey::new(admin),
            mint_address: Pubkey::new(mint_address),
            sol_account: Pubkey::new(sol_account),

            // reserved
            future_address1: Pubkey::new(future_address1),
            future_address2: Pubkey::new(future_address2),
            future_address3: Pubkey::new(future_address3),
            future_address4: Pubkey::new(future_address4),
            future_address5: Pubkey::new(future_address5),

            // proposal nonce
            proposal_nonce: u32::from_le_bytes(*proposal_nonce),
            //creation time
            created_on: i64::from_le_bytes(*created_on),
            // member data struct
            members: member_dser,

            random_id: random_id_deser,

            child_index: 0,
            member_lock_index: u32::from_le_bytes(*member_lock_index),
            reserved: [0; 32],
        })
    }

    fn pack_into_slice(&self, dst: &mut [u8]) {
        // msg!("STATE (Squad Account): pack_into_slice");
        let dst = array_mut_ref![dst, 0, SQUAD_TOTAL_BYTES];

        let (
            is_initialized_dst,
            open_dst,
            emergency_lock_dst,
            // typical settings
            allocation_type_dst,
            vote_support_dst,
            vote_quorum_dst,
            core_threshold_dst,
            // squad_name_len,
            squad_name_dst,
            description_dst,
            token_dst,
            // future settings placeholders
            _future_setting_1_dst,
            _future_setting_2_dst,
            _future_setting_3_dst,
            _future_setting_4_dst,
            _future_setting_5_dst,
            // misc address for squad specific settings
            // admin address for draft mode (open=true) only
            admin_dst,
            mint_address_dst,
            sol_account_dst,
            _future_address1_dst,
            _future_address2_dst,
            _future_address3_dst,
            _future_address4_dst,
            _future_address5_dst,
            proposal_nonce_dst,
            created_on_dst,
            members_len,
            members_dst,
            random_id_dst,
            _child_index_dst,
            member_lock_index_dst,
            _reserved,
        ) = mut_array_refs![
            dst,
            SQUAD_SETTING_BYTES, // is_initialized
            SQUAD_SETTING_BYTES, // open
            SQUAD_SETTING_BYTES, // emergency_lock (DEPRECATED)
            SQUAD_SETTING_BYTES, // allocation_type
            SQUAD_SETTING_BYTES, // vote_support
            SQUAD_SETTING_BYTES, // vote_quorum
            SQUAD_SETTING_BYTES, // core_threshold
            SQUAD_NAME_BYTES,    // bytes for the name
            SQUAD_DESCRIPTION_BYTES,
            SQUAD_TOKEN_BYTES,
            SQUAD_SETTING_BYTES,                              // future_setting_1
            SQUAD_SETTING_BYTES,                              // future_setting_2
            SQUAD_SETTING_BYTES,                              // future_setting_3
            SQUAD_SETTING_BYTES,                              // future_setting_4
            SQUAD_SETTING_BYTES,                              // future_setting_5
            PUBLIC_KEY_BYTES,                                 // admin
            PUBLIC_KEY_BYTES,                                 // mint pda
            PUBLIC_KEY_BYTES,                                 // sol pda
            PUBLIC_KEY_BYTES,                                 // future_address 1
            PUBLIC_KEY_BYTES,                                 // future_address 2
            PUBLIC_KEY_BYTES,                                 // future_address 3
            PUBLIC_KEY_BYTES,                                 // future_address 4
            PUBLIC_KEY_BYTES,                                 // future_address 5
            PROPOSAL_NONCE_BYTES,                             // proposal index
            TIMESTAMP_BYTES,                                  // created_on bytes
            MEMBER_LENGTH_BYTES,                              // bytes for the length num
            ((PUBLIC_KEY_BYTES * 2) * SQUAD_MAX_MEMBERS) + 4, //bytes for the members BTREE itself
            SQUAD_RANDOM_ID_BYTES,
            CHILD_INDEX_BYTES,
            MEMBER_LOCK_BYTES,
            SQUAD_RESERVED_BYTES // reserved for future
        ];

        let Squad {
            is_initialized,
            open,
            emergency_lock,
            // typical settings
            allocation_type,
            vote_support,
            vote_quorum,
            core_threshold,
            squad_name,
            description,
            token,

            // future settings placeholders
            future_setting_1: _,
            future_setting_2: _,
            future_setting_3: _,
            future_setting_4: _,
            future_setting_5: _,

            // misc address for squad specific settings
            // admin address for draft mode (open=true) only
            admin,
            mint_address,
            sol_account,
            future_address1: _,
            future_address2: _,
            future_address3: _,
            future_address4: _,
            future_address5: _,
            proposal_nonce,
            created_on,
            members,
            random_id,
            child_index: _,
            member_lock_index,
            reserved: _,
        } = self;

        is_initialized_dst[0] = *is_initialized as u8;
        open_dst[0] = *open as u8;
        emergency_lock_dst[0] = *emergency_lock as u8;
        *allocation_type_dst = allocation_type.to_le_bytes();
        *vote_support_dst = vote_support.to_le_bytes();
        *vote_quorum_dst = vote_quorum.to_le_bytes();
        *core_threshold_dst = core_threshold.to_le_bytes();
        *created_on_dst = created_on.to_le_bytes();
        admin_dst.copy_from_slice(admin.as_ref());
        mint_address_dst.copy_from_slice(mint_address.as_ref());
        sol_account_dst.copy_from_slice(sol_account.as_ref());

        // pack the squad members
        let members_ser = members.try_to_vec().unwrap();
        members_len[..].copy_from_slice(&transform_u32_to_array_of_u8(members_ser.len() as u32));
        members_dst[..members_ser.len()].copy_from_slice(&members_ser);

        // pack the squad name
        let squad_name_ser = squad_name.as_bytes();
        squad_name_dst[..squad_name_ser.len()].copy_from_slice(squad_name_ser);
        description_dst[..description.len()].copy_from_slice(description.as_bytes());
        token_dst[..token.len()].copy_from_slice(token.as_bytes());

        random_id_dst[..random_id.len()].copy_from_slice(random_id.as_bytes());

        *proposal_nonce_dst = proposal_nonce.to_le_bytes();
        *member_lock_index_dst = member_lock_index.to_le_bytes();
        // when packing we can ignore the future stuff
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // use chrono;
    use solana_program::pubkey::Pubkey;

    #[test]
    fn squad_member_size_est() {
        let mut members = BTreeMap::<Pubkey, Member>::new();
        members.insert(
            Pubkey::new_unique(),
            Member {
                equity_token_account: Pubkey::new_unique(),
            },
        );
        members.insert(
            Pubkey::new_unique(),
            Member {
                equity_token_account: Pubkey::new_unique(),
            },
        );
        members.insert(
            Pubkey::new_unique(),
            Member {
                equity_token_account: Pubkey::new_unique(),
            },
        );
        let members_enc = members.try_to_vec().unwrap();
        println!("members length: {:?}", members_enc.len());
    }

    #[test]
    fn squad_build_from_empty() {
        let squad_arr: [u8; SQUAD_TOTAL_BYTES] = [0; SQUAD_TOTAL_BYTES];
        let _squad_info = Squad::unpack_unchecked(&squad_arr);
    }

    #[test]
    fn squad_build_from_struct() {
        let squad_info = Squad {
            is_initialized: true,

            /// whether or not the owner can still make changes (draft mode)
            open: false,
            emergency_lock: false,

            /// typical settings
            allocation_type: 1,
            vote_support: 50,
            vote_quorum: 50,
            core_threshold: 20,
            squad_name: String::from("THIS IS MY SQUAD"),
            description: String::from("THIS IS A TEST DESCRIPTION"),
            token: String::from("TOKENS"),
            // future settings placeholders
            future_setting_1: 0,
            future_setting_2: 0,
            future_setting_3: 0,
            future_setting_4: 0,
            future_setting_5: 0,

            /// misc address for squad specific settings
            // admin address for draft mode (open=true) only
            admin: Pubkey::new_unique(),
            sol_account: Pubkey::new_unique(),
            mint_address: Pubkey::new_unique(),

            future_address1: Pubkey::new_unique(),
            future_address2: Pubkey::new_unique(),
            future_address3: Pubkey::new_unique(),
            future_address4: Pubkey::new_unique(),
            future_address5: Pubkey::new_unique(),

            random_id: String::from("1234567890"),

            child_index: 0,

            /// the squad member list
            members: BTreeMap::<Pubkey, Member>::new(),
            proposal_nonce: 0,
            member_lock_index: 0,
            created_on: 0,
            reserved: [0; 32],
        };
        let mut squad_dst: [u8; SQUAD_TOTAL_BYTES] = [0; SQUAD_TOTAL_BYTES];
        let _packed_squad = Squad::pack(squad_info, &mut squad_dst);
    }
}
