use crate::error::ElusivError;
use crate::macros::guard;
use crate::state::program_account::{PDAAccount, PDAOffset};
use crate::token::{elusiv_token, Lamports, SPLToken, Token};
use solana_program::instruction::Instruction;
use solana_program::program::invoke;
use solana_program::program_pack::Pack;
use solana_program::pubkey::Pubkey;
use solana_program::sysvar::instructions;
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program_error::ProgramError, rent::Rent,
    sysvar::Sysvar,
};
use spl_associated_token_account::get_associated_token_address;

pub use elusiv_utils::*;

/// No-operation instruction
pub fn nop() -> solana_program::entrypoint::ProgramResult {
    Ok(())
}

pub trait InstructionsSysvar {
    fn current_index(&self) -> Result<u16, ProgramError>;
    fn instruction_at_index(&self, index: usize) -> Result<Instruction, ProgramError>;

    fn find_instruction_count(&self) -> Result<usize, ProgramError> {
        let mut index = self.current_index()? as usize;
        while self.instruction_at_index(index).is_ok() {
            index += 1;
        }
        Ok(index)
    }
}

pub struct DefaultInstructionsSysvar<'a, 'b>(pub &'a AccountInfo<'b>);

impl<'a, 'b> InstructionsSysvar for DefaultInstructionsSysvar<'a, 'b> {
    fn current_index(&self) -> Result<u16, ProgramError> {
        instructions::load_current_index_checked(self.0)
    }

    fn instruction_at_index(&self, index: usize) -> Result<Instruction, ProgramError> {
        instructions::load_instruction_at_checked(index, self.0)
    }
}

pub fn transfer_token<'a>(
    source: &AccountInfo<'a>,
    source_token_account: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    token: Token,
) -> ProgramResult {
    match token {
        Token::Lamports(lamports) => {
            transfer_with_system_program(source, destination, token_program, lamports.0)
        }
        Token::SPLToken(SPLToken { amount, .. }) => transfer_with_token_program(
            source,
            source_token_account,
            destination,
            token_program,
            amount,
            None,
        ),
    }
}

pub fn transfer_token_from_pda<'a, T: PDAAccount>(
    source: &AccountInfo<'a>,
    source_token_account: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    token: Token,
    pda_pubkey: Option<Pubkey>,
    pda_offset: PDAOffset,
) -> ProgramResult {
    guard!(*source.owner == crate::ID, ElusivError::InvalidAccount);

    match token {
        Token::Lamports(lamports) => {
            transfer_lamports_from_pda_checked(source, destination, lamports.0)
        }
        Token::SPLToken(SPLToken { amount, .. }) => {
            let bump = T::get_bump(source);
            let seeds = T::signers_seeds(pda_pubkey, pda_offset, bump);
            let signers_seeds = signers_seeds!(seeds);

            transfer_with_token_program(
                source,
                source_token_account,
                destination,
                token_program,
                amount,
                Some(&[&signers_seeds]),
            )
        }
    }
}

fn transfer_with_token_program<'a>(
    source: &AccountInfo<'a>,
    source_token_account: &AccountInfo<'a>,
    destination_token_account: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    amount: u64,
    signers_seeds: Option<&[&[&[u8]]]>,
) -> ProgramResult {
    guard!(
        *token_program.key == spl_token::ID,
        ElusivError::InvalidAccount
    );

    guard!(
        *source_token_account.owner == spl_token::ID,
        ElusivError::InvalidAccount
    ); // redundant
    guard!(
        *destination_token_account.owner == spl_token::ID,
        ElusivError::InvalidAccount
    );

    let instruction = spl_token::instruction::transfer(
        &spl_token::id(),
        source_token_account.key,
        destination_token_account.key,
        source.key,
        &[source.key],
        amount,
    )?;

    if let Some(signers_seeds) = signers_seeds {
        solana_program::program::invoke_signed(
            &instruction,
            &[
                source.clone(),
                source_token_account.clone(),
                destination_token_account.clone(),
                token_program.clone(),
            ],
            signers_seeds,
        )
    } else {
        solana_program::program::invoke(
            &instruction,
            &[
                source.clone(),
                source_token_account.clone(),
                destination_token_account.clone(),
                token_program.clone(),
            ],
        )
    }
}

pub fn create_associated_token_account<'a>(
    payer: &AccountInfo<'a>,
    wallet_account: &AccountInfo<'a>,
    associated_token_account: &AccountInfo<'a>,
    mint_account: &AccountInfo<'a>,

    token_id: u16,
) -> Result<(), ProgramError> {
    invoke(
        &spl_associated_token_account::instruction::create_associated_token_account(
            payer.key,
            wallet_account.key,
            &elusiv_token(token_id)?.mint,
            &spl_token::ID,
        ),
        &[
            payer.clone(),
            associated_token_account.clone(),
            wallet_account.clone(),
            mint_account.clone(),
        ],
    )
}

pub fn program_token_account_address<A: PDAAccount>(
    token_id: u16,
    offset: PDAOffset,
) -> Result<Pubkey, ProgramError> {
    Ok(get_associated_token_address(
        &A::find(offset).0,
        &elusiv_token(token_id)?.mint,
    ))
}

pub fn verify_program_token_account(
    owner_pda: &AccountInfo,
    token_account: &AccountInfo,
    token_id: u16,
) -> ProgramResult {
    if token_id == 0 {
        guard!(
            owner_pda.key == token_account.key,
            ElusivError::InvalidAccount
        );
    } else {
        let pubkey = get_associated_token_address(owner_pda.key, &elusiv_token(token_id)?.mint);
        guard!(pubkey == *token_account.key, ElusivError::InvalidAccount);
    }

    Ok(())
}

pub fn system_program_account_rent() -> Result<Lamports, ProgramError> {
    #[cfg(test)]
    {
        Ok(Lamports(0))
    }

    #[cfg(not(test))]
    {
        Ok(Lamports(Rent::get()?.minimum_balance(0)))
    }
}

pub fn spl_token_account_rent() -> Result<Lamports, ProgramError> {
    Ok(Lamports(
        Rent::get()?.minimum_balance(spl_token::state::Account::LEN),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        macros::{account_info, test_account_info},
        state::{governor::PoolAccount, proof::VerificationAccount},
        token::TOKENS,
    };
    use solana_program::{pubkey::Pubkey, system_program};

    #[test]
    fn test_transfer_token_from_pda() {
        test_account_info!(non_pda, 0, Pubkey::new_unique());
        account_info!(pda, elusiv::id(), vec![0]);
        account_info!(token_program, spl_token::id(), vec![]);
        test_account_info!(src, 0, spl_token::id());
        test_account_info!(dst, 0, spl_token::id());

        assert_eq!(
            transfer_token_from_pda::<PoolAccount>(
                &non_pda,
                &src,
                &dst,
                &token_program,
                Token::new(1, 100),
                None,
                None
            ),
            Err(ElusivError::InvalidAccount.into())
        );

        assert_eq!(
            transfer_token_from_pda::<PoolAccount>(
                &pda,
                &src,
                &dst,
                &token_program,
                Token::new(1, 100),
                None,
                None
            ),
            Ok(())
        );
    }

    #[test]
    fn test_transfer_with_system_program() {
        test_account_info!(source, 0);
        test_account_info!(destination, 0);

        account_info!(system_program, system_program::id(), vec![]);
        account_info!(invalid_system_program, Pubkey::new_unique(), vec![]);

        assert_eq!(
            transfer_with_system_program(&source, &destination, &invalid_system_program, 100),
            Err(ProgramError::IncorrectProgramId)
        );

        assert_eq!(
            transfer_with_system_program(&source, &destination, &system_program, 100),
            Ok(())
        );
    }

    #[test]
    fn test_transfer_with_token_program() {
        test_account_info!(source, 0);
        test_account_info!(source_token_account, 0, spl_token::id());
        test_account_info!(destination, 0, spl_token::id());

        test_account_info!(invalid_source_token_account, 0);
        test_account_info!(invalid_destination, 0);

        account_info!(token_program, spl_token::id(), vec![]);
        account_info!(invalid_token_program, Pubkey::new_unique(), vec![]);

        assert_eq!(
            transfer_with_token_program(
                &source,
                &source_token_account,
                &destination,
                &invalid_token_program,
                100,
                None,
            ),
            Err(ElusivError::InvalidAccount.into())
        );

        assert_eq!(
            transfer_with_token_program(
                &source,
                &invalid_source_token_account,
                &destination,
                &token_program,
                100,
                None,
            ),
            Err(ElusivError::InvalidAccount.into())
        );

        assert_eq!(
            transfer_with_token_program(
                &source,
                &source_token_account,
                &invalid_destination,
                &token_program,
                100,
                None,
            ),
            Err(ElusivError::InvalidAccount.into())
        );

        assert_eq!(
            transfer_with_token_program(
                &source,
                &source_token_account,
                &destination,
                &token_program,
                100,
                None,
            ),
            Ok(())
        );
    }

    #[test]
    fn test_open_pda_account_with_offset() {
        test_account_info!(payer, 0);
        let (pubkey, bump) = VerificationAccount::find(Some(3));
        account_info!(pda_account, pubkey, vec![]);

        assert_eq!(
            open_pda_account_with_offset::<VerificationAccount>(
                &crate::id(),
                &payer,
                &pda_account,
                2,
                None
            ),
            Err(ProgramError::InvalidSeeds)
        );

        assert_eq!(
            open_pda_account_with_offset::<VerificationAccount>(
                &crate::id(),
                &payer,
                &pda_account,
                3,
                None
            ),
            Ok(())
        );

        // Using bump:
        assert_eq!(
            open_pda_account_with_offset::<VerificationAccount>(
                &crate::id(),
                &payer,
                &pda_account,
                2,
                Some(bump)
            ),
            Err(ProgramError::InvalidSeeds)
        );

        assert_eq!(
            open_pda_account_with_offset::<VerificationAccount>(
                &crate::id(),
                &payer,
                &pda_account,
                3,
                Some(0)
            ),
            Err(ProgramError::InvalidSeeds)
        );

        assert_eq!(
            open_pda_account_with_offset::<VerificationAccount>(
                &crate::id(),
                &payer,
                &pda_account,
                3,
                Some(bump)
            ),
            Ok(())
        );
    }

    #[test]
    fn test_open_pda_account_without_offset() {
        test_account_info!(payer, 0);

        let (pubkey, bump) = VerificationAccount::find(None);
        account_info!(pda_account, pubkey, vec![]);

        let (invalid_pubkey, invalid_bump) = VerificationAccount::find(Some(0));
        account_info!(invalid_pda_account, invalid_pubkey, vec![]);

        assert_eq!(
            open_pda_account_without_offset::<VerificationAccount>(
                &crate::id(),
                &payer,
                &invalid_pda_account,
                None
            ),
            Err(ProgramError::InvalidSeeds)
        );

        assert_eq!(
            open_pda_account_without_offset::<VerificationAccount>(
                &crate::id(),
                &payer,
                &pda_account,
                None
            ),
            Ok(())
        );

        // Using bump:
        assert_eq!(
            open_pda_account_without_offset::<VerificationAccount>(
                &crate::id(),
                &payer,
                &invalid_pda_account,
                Some(bump)
            ),
            Err(ProgramError::InvalidSeeds)
        );

        assert_eq!(
            open_pda_account_without_offset::<VerificationAccount>(
                &crate::id(),
                &payer,
                &invalid_pda_account,
                Some(invalid_bump)
            ),
            Err(ProgramError::InvalidSeeds)
        );

        // Invalid bump can be supplied for None due to FIRST_PDA
        assert_eq!(
            open_pda_account_without_offset::<VerificationAccount>(
                &crate::id(),
                &payer,
                &pda_account,
                Some(123)
            ),
            Ok(())
        );

        assert_eq!(
            open_pda_account_without_offset::<VerificationAccount>(
                &crate::id(),
                &payer,
                &pda_account,
                Some(bump)
            ),
            Ok(())
        );
    }

    #[test]
    fn test_transfer_lamports_from_pda() {
        account_info!(pda, Pubkey::new_unique(), vec![]);
        account_info!(recipient, Pubkey::new_unique(), vec![]);

        unsafe {
            // Underflow
            let balance = pda.lamports();
            assert_eq!(
                transfer_lamports_from_pda(&pda, &recipient, balance + 1),
                Err(MATH_ERR)
            );

            // Overflow
            assert_eq!(
                transfer_lamports_from_pda(&pda, &recipient, u64::MAX),
                Err(MATH_ERR)
            );

            // Valid amount
            assert_eq!(
                transfer_lamports_from_pda(&pda, &recipient, balance),
                Ok(())
            );
            assert_eq!(pda.lamports(), 0);
            assert_eq!(recipient.lamports(), balance * 2);
        }
    }

    #[test]
    fn test_close_account() {
        account_info!(account, Pubkey::new_unique(), vec![]);
        account_info!(payer, Pubkey::new_unique(), vec![]);

        let start_balance = account.lamports();

        assert_eq!(close_account(&payer, &account), Ok(()));

        assert_eq!(account.lamports(), 0);
        assert_ne!(account.lamports(), start_balance);
        assert_eq!(payer.lamports(), start_balance * 2);
    }

    #[test]
    fn test_verify_program_token_account() {
        let pk_pool_0 = get_associated_token_address(&PoolAccount::find(None).0, &TOKENS[1].mint);
        let pk_pool_1 = get_associated_token_address(&PoolAccount::find(None).0, &TOKENS[2].mint);

        account_info!(pool, PoolAccount::find(None).0, vec![]);
        account_info!(token_account0, pk_pool_0, vec![]);
        account_info!(token_account1, pk_pool_1, vec![]);

        assert_eq!(verify_program_token_account(&pool, &pool, 0), Ok(()));
        assert_eq!(
            verify_program_token_account(&pool, &token_account0, 1),
            Ok(())
        );
        assert_eq!(
            verify_program_token_account(&pool, &token_account1, 1),
            Err(ElusivError::InvalidAccount.into())
        );

        assert_eq!(
            verify_program_token_account(&pool, &token_account1, 2),
            Ok(())
        );
        assert_eq!(
            verify_program_token_account(&pool, &token_account0, 2),
            Err(ElusivError::InvalidAccount.into())
        );
    }
}
