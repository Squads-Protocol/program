/* SQUADS */
// inside error.rs
use solana_program::program_error::ProgramError;
use thiserror::Error;

#[derive(Error, Debug, Copy, Clone)]
pub enum SquadError {
    /// Invalid instruction
    #[error("Invalid Instruction")]
    InvalidInstruction,
    /// Invalid instruction
    #[error("Squad account is NOT Rent exempt!")]
    NotRentExempt,
    #[error("Squad already exists")]
    SquadAlreadyExists,
}

impl From<SquadError> for ProgramError {
    fn from(e: SquadError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
