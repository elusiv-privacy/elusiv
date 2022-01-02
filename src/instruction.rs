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
    /// 2. [static] System program
    Deposit {
        /// Deposit amount in Lamports
        amount: u64,
    },

    /// Withdraw SOL
    /// 
    /// Accounts expected:
    /// 0. [signer] Initiator of the withdrawal
    /// 1. [writable] Recipient of the withdrawal
    /// 2. [owned, writable] Bank and storage account
    Withdraw {
        /// Withdrawal amount in Lamports
        amount: u64,
    }
}

impl ElusivInstruction {
    pub fn unpack(data: &[u8]) -> Result<Self, ProgramError> {
        let (&tag, rest) = data
            .split_first()
            .ok_or(ProgramError::InvalidInstructionData)?;

        match tag {
            0 => Self::unpack_deposit(&rest),
            1 => Self::unpack_withdraw(&rest),
            _ => Err(InvalidArgument)
        }
    }

    fn unpack_deposit(data: &[u8]) -> Result<Self, ProgramError> {
        let amount = Self::unpack_u64(&data)?;

        Ok(ElusivInstruction::Deposit{ amount })
    }

    fn unpack_withdraw(data: &[u8]) -> Result<Self, ProgramError> {
        let amount = Self::unpack_u64(&data)?;

        Ok(ElusivInstruction::Withdraw{ amount })
    }

    fn unpack_u64(data: &[u8]) -> Result<u64, ProgramError> {
        let amount = data
            .get(..8)
            .and_then(|slice| slice.try_into().ok())
            .map(u64::from_le_bytes)
            .ok_or(InvalidArgument)?;

        Ok(amount)
    }
}