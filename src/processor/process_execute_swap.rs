use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    instruction::{AccountMeta, Instruction},
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
    rent::Rent,
    system_instruction::{allocate, assign, create_account, transfer},
    sysvar::Sysvar,
};

use crate::*;

use crate::state::squad::AllocationType;
use spl_token::{instruction::initialize_account, state::Account};

mod raydium_constant {
    solana_program::declare_id!("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8");
}

fn swap(
    program_id: &Pubkey,
    amm_id: &Pubkey,
    amm_authority: &Pubkey,
    amm_open_orders: &Pubkey,
    amm_target_orders: &Pubkey,
    pool_coin_token_account: &Pubkey,
    pool_pc_token_account: &Pubkey,
    serum_program_id: &Pubkey,
    serum_market: &Pubkey,
    serum_bids: &Pubkey,
    serum_asks: &Pubkey,
    serum_event_queue: &Pubkey,
    serum_coin_vault_account: &Pubkey,
    serum_pc_vault_account: &Pubkey,
    serum_vault_signer: &Pubkey,
    uer_source_token_account: &Pubkey,
    uer_destination_token_account: &Pubkey,
    user_source_owner: &Pubkey,

    amount_in: u64,
    minimum_amount_out: u64,
) -> Result<Instruction, ProgramError> {
    let mut data = vec![9];
    data.extend_from_slice(&amount_in.to_le_bytes());
    data.extend_from_slice(&minimum_amount_out.to_le_bytes());

    let accounts = vec![
        // spl token
        AccountMeta::new_readonly(spl_token::id(), false),
        // amm
        AccountMeta::new(*amm_id, false),
        AccountMeta::new_readonly(*amm_authority, false),
        AccountMeta::new(*amm_open_orders, false),
        AccountMeta::new(*amm_target_orders, false),
        AccountMeta::new(*pool_coin_token_account, false),
        AccountMeta::new(*pool_pc_token_account, false),
        // serum
        AccountMeta::new_readonly(*serum_program_id, false),
        AccountMeta::new(*serum_market, false),
        AccountMeta::new(*serum_bids, false),
        AccountMeta::new(*serum_asks, false),
        AccountMeta::new(*serum_event_queue, false),
        AccountMeta::new(*serum_coin_vault_account, false),
        AccountMeta::new(*serum_pc_vault_account, false),
        AccountMeta::new_readonly(*serum_vault_signer, false),
        // user
        AccountMeta::new(*uer_source_token_account, false),
        AccountMeta::new(*uer_destination_token_account, false),
        AccountMeta::new_readonly(*user_source_owner, true),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

pub fn process_execute_swap(
    accounts: &[AccountInfo],
    amount: u64,
    amount_out: u64,
    allocation_type: u8,
    random_id: String,
    program_id: &Pubkey,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let initializer = next_account_info(account_info_iter)?; // initializer
    let squad_account = next_account_info(account_info_iter)?;
    if allocation_type == AllocationType::TeamCoordination as u8 {
        next_account_info(account_info_iter)?; // squad_mint_account
    }
    next_account_info(account_info_iter)?; // proposal_account
    let source_mint = next_account_info(account_info_iter)?;
    let destination_mint = next_account_info(account_info_iter)?;
    let system_program_account = next_account_info(account_info_iter)?; // system_program_account
    let token_program_account = next_account_info(account_info_iter)?; // token_program_account
    next_account_info(account_info_iter)?; // associated_program_account
    let rent_account = next_account_info(account_info_iter)?; // rent_account
    let sol_account = next_account_info(account_info_iter)?;
    let source_account = next_account_info(account_info_iter)?;
    let destination_account = next_account_info(account_info_iter)?;
    let wsol_account = next_account_info(account_info_iter)?;
    let wsol_mint = next_account_info(account_info_iter)?;
    // accounts for swap
    let raydium_id = next_account_info(account_info_iter)?;
    let amm_id = next_account_info(account_info_iter)?;
    let amm_authority = next_account_info(account_info_iter)?;
    let amm_open_orders = next_account_info(account_info_iter)?;
    let amm_target_orders = next_account_info(account_info_iter)?;
    let pool_coin_token_account = next_account_info(account_info_iter)?;
    let pool_pc_token_account = next_account_info(account_info_iter)?;
    let serum_program_id = next_account_info(account_info_iter)?;
    let serum_market = next_account_info(account_info_iter)?;
    let serum_bids = next_account_info(account_info_iter)?;
    let serum_asks = next_account_info(account_info_iter)?;
    let serum_event_queue = next_account_info(account_info_iter)?;
    let serum_coin_vault_account = next_account_info(account_info_iter)?;
    let serum_pc_vault_account = next_account_info(account_info_iter)?;
    let serum_vault_signer = next_account_info(account_info_iter)?;

    let rent = &Rent::from_account_info(rent_account)?;

    let (sol_address, sol_bump_seed) = get_sol_address_with_seed(&squad_account.key, program_id);
    let sol_signer_seeds: &[&[_]] = &[
        &squad_account.key.to_bytes(),
        b"!squadsol",
        &[sol_bump_seed],
    ];

    if sol_account.key != &sol_address {
        return Err(ProgramError::InvalidAccountData);
    }

    // Check that the program is raydium
    if raydium_id.key != &raydium_constant::id() {
        return Err(ProgramError::InvalidAccountData);
    }

    // Check amm is owner by raydium
    if amm_id.owner != raydium_id.key {
        return Err(ProgramError::InvalidAccountData);
    }

    // Check that the serum_market is owned by the serum program
    if serum_market.owner != serum_program_id.key {
        return Err(ProgramError::InvalidAccountData);
    }

    // Check pool_coin info
    let pool_coin_token_account_info =
        Account::unpack_unchecked(&pool_coin_token_account.data.borrow())?;
    if pool_coin_token_account_info.owner != *amm_authority.key {
        return Err(ProgramError::InvalidAccountData);
    }
    if pool_coin_token_account_info.mint != *source_mint.key
        && pool_coin_token_account_info.mint != *destination_mint.key
    {
        return Err(ProgramError::InvalidAccountData);
    }

    // Check pool_pc info
    let pool_pc_token_account_info =
        Account::unpack_unchecked(&pool_pc_token_account.data.borrow())?;
    if pool_pc_token_account_info.owner != *amm_authority.key {
        return Err(ProgramError::InvalidAccountData);
    }
    if pool_pc_token_account_info.mint != *source_mint.key
        && pool_pc_token_account_info.mint != *destination_mint.key
    {
        return Err(ProgramError::InvalidAccountData);
    }

    // Check that amm_open_orders is owned by serum
    if amm_open_orders.owner != serum_program_id.key {
        return Err(ProgramError::InvalidAccountData);
    }

    // Check that amm_target_orders is owned by raydium
    if amm_target_orders.owner != raydium_id.key {
        return Err(ProgramError::InvalidAccountData);
    }

    if serum_bids.owner != serum_program_id.key {
        return Err(ProgramError::InvalidAccountData);
    }

    if serum_asks.owner != serum_program_id.key {
        return Err(ProgramError::InvalidAccountData);
    }

    let serum_coin_vault_account_info =
        Account::unpack_unchecked(&serum_coin_vault_account.data.borrow())?;
    if serum_coin_vault_account_info.owner != *serum_vault_signer.key {
        return Err(ProgramError::InvalidAccountData);
    }
    if serum_coin_vault_account_info.mint != *source_mint.key
        && serum_coin_vault_account_info.mint != *destination_mint.key
    {
        return Err(ProgramError::InvalidAccountData);
    }

    let serum_pc_vault_account_info =
        Account::unpack_unchecked(&serum_pc_vault_account.data.borrow())?;
    if serum_pc_vault_account_info.owner != *serum_vault_signer.key {
        return Err(ProgramError::InvalidAccountData);
    }
    if serum_pc_vault_account_info.mint != *source_mint.key
        && serum_pc_vault_account_info.mint != *destination_mint.key
    {
        return Err(ProgramError::InvalidAccountData);
    }

    if pool_coin_token_account_info.mint != serum_coin_vault_account_info.mint {
        return Err(ProgramError::InvalidAccountData);
    }

    if pool_pc_token_account_info.mint != serum_pc_vault_account_info.mint {
        return Err(ProgramError::InvalidAccountData);
    }

    let (_wsol_address, wsol_bump_seed) =
        get_wsol_address_with_seed(&sol_account.key, &random_id, program_id);
    let wsol_signer_seeds: &[&[_]] = &[
        &sol_account.key.to_bytes(),
        random_id.as_bytes(),
        b"!wsol",
        &[wsol_bump_seed],
    ];

    let instruction = swap(
        &raydium_id.key,
        &amm_id.key,
        &amm_authority.key,
        &amm_open_orders.key,
        &amm_target_orders.key,
        &pool_coin_token_account.key,
        &pool_pc_token_account.key,
        &serum_program_id.key,
        &serum_market.key,
        &serum_bids.key,
        &serum_asks.key,
        &serum_event_queue.key,
        &serum_coin_vault_account.key,
        &serum_pc_vault_account.key,
        &serum_vault_signer.key,
        &source_account.key,
        &destination_account.key,
        &sol_account.key,
        amount,
        amount_out,
    )?;

    if source_mint.key == &spl_token::native_mint::id()
        || destination_mint.key == &spl_token::native_mint::id()
    {
        // DoS check
        let rent_exempt_lamports = rent
            .minimum_balance(spl_token::state::Account::get_packed_len())
            .max(1);
        if wsol_account.lamports() > 0 {
            let top_up_lamports = rent_exempt_lamports.saturating_sub(wsol_account.lamports());

            if top_up_lamports > 0 {
                invoke(
                    &transfer(initializer.key, wsol_account.key, top_up_lamports),
                    &[
                        initializer.clone(),
                        wsol_account.clone(),
                        system_program_account.clone(),
                    ],
                )?;
            }

            invoke_signed(
                &allocate(wsol_account.key, VoteReceipt::get_packed_len() as u64),
                &[wsol_account.clone(), system_program_account.clone()],
                &[&wsol_signer_seeds],
            )?;

            invoke_signed(
                &assign(wsol_account.key, program_id),
                &[wsol_account.clone(), system_program_account.clone()],
                &[&wsol_signer_seeds],
            )?;
        } else {
            // Create the wSOL account
            invoke_signed(
                &create_account(
                    initializer.key,
                    &wsol_account.key,
                    1.max(rent.minimum_balance(spl_token::state::Account::get_packed_len())),
                    spl_token::state::Account::get_packed_len() as u64,
                    &token_program_account.key,
                ),
                &[
                    initializer.clone(),
                    wsol_account.clone(),
                    system_program_account.clone(),
                ],
                &[&wsol_signer_seeds],
            )?;
        }

        // If the source needs to be wSOL we need to fund it
        if source_mint.key == &spl_token::native_mint::id() {
            let transfer_ix = transfer(&sol_address, &wsol_account.key, amount);
            invoke_signed(
                &transfer_ix,
                &[
                    sol_account.clone(),
                    wsol_account.clone(),
                    system_program_account.clone(),
                ],
                &[&sol_signer_seeds],
            )?;
        }

        invoke_signed(
            &initialize_account(
                token_program_account.key,
                wsol_account.key,
                &spl_token::native_mint::id(),
                sol_account.key,
            )?,
            &[
                token_program_account.clone(),
                wsol_account.clone(),
                sol_account.clone(),
                wsol_mint.clone(),
                rent_account.clone(),
            ],
            &[wsol_signer_seeds],
        )?;
    }

    invoke_signed(
        &instruction,
        &[
            raydium_id.clone(),
            amm_id.clone(),
            amm_authority.clone(),
            amm_open_orders.clone(),
            amm_target_orders.clone(),
            pool_coin_token_account.clone(),
            pool_pc_token_account.clone(),
            serum_program_id.clone(),
            serum_market.clone(),
            serum_bids.clone(),
            serum_asks.clone(),
            serum_event_queue.clone(),
            serum_coin_vault_account.clone(),
            serum_pc_vault_account.clone(),
            serum_vault_signer.clone(),
            source_account.clone(),
            destination_account.clone(),
            sol_account.clone(),
        ],
        &[&sol_signer_seeds],
    )?;

    if source_mint.key == &spl_token::native_mint::id()
        || destination_mint.key == &spl_token::native_mint::id()
    {
        invoke_signed(
            &spl_token::instruction::close_account(
                &spl_token::id(),
                &wsol_account.key,
                &sol_account.key,
                &sol_account.key,
                &[],
            )?,
            &[
                sol_account.clone(),
                wsol_account.clone(),
                system_program_account.clone(),
            ],
            &[&sol_signer_seeds],
        )?;
    }
    Ok(())
}
