use solana_program::{
    program_error::{
        ProgramError,
        ProgramError::InvalidArgument,
    },
};
use std::convert::TryInto;

pub enum ElusivInstruction {
    /// Deposits SOL 
    /// 
    /// Accounts expected:
    /// 0. [signer, writable] Depositor account
    /// 1. [owned, writable] Bank and storage account
    Deposit {
        /// Deposit amount in Lamports
        amount: u64,
    },

    /// Withdraw SOL
    /// 
    /// Accounts expected:
    /// 0. [signer, writable] Recipient of the withdrawal
    /// 1. [owned, writable] Bank and storage account
    Withdraw
}

impl ElusivInstruction {
    pub fn unpack(data: &[u8]) -> Result<Self, ProgramError> {
        let (&tag, rest) = data
            .split_first()
            .ok_or(ProgramError::InvalidInstructionData)?;

        return match tag {
            0 => Self::unpack_deposit(&rest),
            1 => Self::unpack_withdraw(),
            _ => Err(InvalidArgument)
        };
    }

    fn unpack_deposit(data: &[u8]) -> Result<Self, ProgramError> {
        let amount = data
            .get(..8)
            .and_then(|slice| slice.try_into().ok())
            .map(u64::from_le_bytes)
            .ok_or(InvalidArgument)?;

        Ok(ElusivInstruction::Deposit{ amount })
    }

    fn unpack_withdraw() -> Result<Self, ProgramError> {
        Ok(ElusivInstruction::Withdraw)
    }
}