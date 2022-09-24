use solana_program::program::invoke;
use solana_program::program_pack::Pack;
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    system_program,
    program_error::ProgramError,
    system_instruction,
    program::invoke_signed,
    rent::Rent,
    sysvar::Sysvar,
};
use crate::error::ElusivError::{
    InvalidAccount,
    InvalidInstructionData,
    InsufficientFunds
};
use crate::macros::{guard, pda_account};
use crate::state::governor::{PoolAccount, FeeCollectorAccount};
use crate::state::program_account::{PDAAccount, ProgramAccount, PDAOffset};
use crate::token::{Token, Lamports, TokenAuthorityAccount, TOKENS, SPLToken, SPL_TOKEN_COUNT, elusiv_token};

pub use elusiv_utils::*;

pub fn transfer_token<'a>(
    source: &AccountInfo<'a>,
    source_token_account: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    token: Token,
) -> ProgramResult {
    match token {
        Token::Lamports(lamports) => {
            transfer_with_system_program(
                source,
                destination,
                token_program,
                lamports,
            )
        }
        Token::SPLToken(SPLToken { amount, .. }) => {
            transfer_with_token_program(
                source,
                source_token_account,
                destination,
                token_program,
                amount,
                &[],
            )
        }
    }
}

pub fn transfer_token_from_pda<'a, T: PDAAccount>(
    source: &AccountInfo<'a>,
    source_token_account: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    token: Token,
    pda_offset: PDAOffset,
) -> ProgramResult {
    guard!(*source.owner == crate::ID, InvalidAccount);

    match token {
        Token::Lamports(lamports) => {
            transfer_lamports_from_pda_checked(
                source,
                destination,
                lamports.0,
            )
        }
        Token::SPLToken(SPLToken { amount, .. }) => {
            let bump = T::get_bump(source);
            let seeds = T::signers_seeds(pda_offset, bump);
            let signers_seeds = signers_seeds!(seeds);

            transfer_with_token_program(
                source,
                source_token_account,
                destination,
                token_program,
                amount,
                &[&signers_seeds],
            )
        }
    }
}

pub fn transfer_with_system_program<'a>(
    source: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    lamports: Lamports,
) -> ProgramResult {
    guard!(*system_program.key == system_program::ID, InvalidAccount);

    let instruction = solana_program::system_instruction::transfer(
        source.key,
        destination.key,
        lamports.0,
    );
    
    solana_program::program::invoke(
        &instruction,
        &[
            source.clone(),
            destination.clone(),
            system_program.clone(),
        ],
    )    
}

fn transfer_with_token_program<'a>(
    source: &AccountInfo<'a>,
    source_token_account: &AccountInfo<'a>,
    destination_token_account: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    amount: u64,
    signers_seeds: &[&[&[u8]]],
) -> ProgramResult {
    guard!(*token_program.key == spl_token::ID, InvalidAccount);

    guard!(*source_token_account.owner == spl_token::ID, InvalidAccount);   // redundant
    guard!(*destination_token_account.owner == spl_token::ID, InvalidAccount);

    let instruction = spl_token::instruction::transfer(
        &spl_token::id(),
        source_token_account.key,
        destination_token_account.key,
        source.key,
        &[source.key],
        amount,
    )?;

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
}

pub fn create_token_account_for_pda_authority<'a, T: PDAAccount>(
    payer: &AccountInfo<'a>,
    pda_account: &AccountInfo<'a>,
    token_account: &AccountInfo<'a>,
    mint_account: &AccountInfo<'a>,
    token_id: u16,
) -> ProgramResult {
    create_token_account(
        payer,
        pda_account,
        token_account,
        mint_account,
        token_id,
    )
}

pub fn create_token_account<'a>(
    payer: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    token_account: &AccountInfo<'a>,
    mint_account: &AccountInfo<'a>,
    token_id: u16,
) -> ProgramResult {
    guard!(token_id > 0 && token_id as usize <= SPL_TOKEN_COUNT, InvalidInstructionData);

    if cfg!(test) {
        return Ok(());
    }

    let space = spl_token::state::Account::LEN;
    let lamports_required = spl_token_account_rent()?.0;
    guard!(payer.lamports() >= lamports_required, InsufficientFunds);

    invoke_signed(
        &system_instruction::create_account(
            payer.key,
            token_account.key,
            lamports_required,
            space.try_into().unwrap(),
            &spl_token::id(),
        ),
        &[
            payer.clone(),
            token_account.clone(),
        ],
        &[]
    )?;

    let token = TOKENS[token_id as usize];

    invoke_signed(
        &spl_token::instruction::initialize_account3(
            &spl_token::id(),
            token_account.key,
            &token.mint,
            authority.key,
        ).unwrap(),
        &[
            token_account.clone(),
            mint_account.clone(),
            authority.clone(),
        ],
        &[],
    )
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

macro_rules! verify_token_account {
    ($fn_id: ident, $ty: ty) => {
        pub fn $fn_id(
            owner_pda: &AccountInfo,
            token_account: &AccountInfo,
            token_id: u16,
        ) -> ProgramResult {
            if token_id == 0 {
                guard!(owner_pda.key == token_account.key, InvalidAccount);
            } else {
                pda_account!(owner, $ty, owner_pda);
                owner.enforce_token_account(token_id, token_account)?;
            }

            Ok(())
        }
    };
}

verify_token_account!(verify_pool, PoolAccount);
verify_token_account!(verify_fee_collector, FeeCollectorAccount);

pub fn spl_token_account_rent() -> Result<Lamports, ProgramError> {
    Ok(Lamports(Rent::get()?.minimum_balance(spl_token::state::Account::LEN)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{macros::{test_account_info, account}, proof::VerificationAccount, bytes::ElusivOption};
    use crate::state::program_account::SizedAccount;
    use assert_matches::assert_matches;
    use solana_program::pubkey::Pubkey;

    #[test]
    #[ignore]
    fn test_transfer_token() {
        panic!()
    }

    #[test]
    fn test_transfer_token_from_pda() {
        test_account_info!(non_pda, 0, Pubkey::new_unique());
        account!(pda, elusiv::id(), vec![0]);
        account!(token_program, spl_token::id(), vec![]);
        test_account_info!(src, 0, spl_token::id());
        test_account_info!(dst, 0, spl_token::id());

        assert_matches!(
            transfer_token_from_pda::<PoolAccount>(&non_pda, &src, &dst, &token_program, Token::new(1, 100), None),
            Err(_)
        );

        assert_matches!(
            transfer_token_from_pda::<PoolAccount>(&pda, &src, &dst, &token_program, Token::new(1, 100), None),
            Ok(_)
        );
    }

    #[test]
    fn test_transfer_with_system_program() {
        test_account_info!(source, 0);
        test_account_info!(destination, 0);

        account!(system_program, system_program::id(), vec![]);
        account!(invalid_system_program, Pubkey::new_unique(), vec![]);

        assert_matches!(
            transfer_with_system_program(&source, &destination, &invalid_system_program, Lamports(100)),
            Err(_)
        );

        assert_matches!(
            transfer_with_system_program(&source, &destination, &system_program, Lamports(100)),
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

        account!(token_program, spl_token::id(), vec![]);
        account!(invalid_token_program, Pubkey::new_unique(), vec![]);

        assert_matches!(
            transfer_with_token_program(
                &source,
                &source_token_account,
                &destination,
                &invalid_token_program,
                100,
                &[],
            ),
            Err(_)
        );

        assert_matches!(
            transfer_with_token_program(
                &source,
                &invalid_source_token_account,
                &destination,
                &token_program,
                100,
                &[],
            ),
            Err(_)
        );

        assert_matches!(
            transfer_with_token_program(
                &source,
                &source_token_account,
                &invalid_destination,
                &token_program,
                100,
                &[],
            ),
            Err(_)
        );

        assert_matches!(
            transfer_with_token_program(
                &source,
                &source_token_account,
                &destination,
                &token_program,
                100,
                &[],
            ),
            Ok(())
        );
    }

    #[test]
    fn test_open_pda_account_with_offset() {
        test_account_info!(payer, 0);
        account!(pda_account, VerificationAccount::find(Some(3)).0, vec![]);

        assert_matches!(
            open_pda_account_with_offset::<VerificationAccount>(&crate::id(), &payer, &pda_account, 2),
            Err(_)
        );

        assert_matches!(
            open_pda_account_with_offset::<VerificationAccount>(&crate::id(), &payer, &pda_account, 3),
            Ok(())
        );
    }

    #[test]
    fn test_open_pda_account_without_offset() {
        test_account_info!(payer, 0);
        account!(pda_account, VerificationAccount::find(None).0, vec![]);
        account!(invalid_pda_account, VerificationAccount::find(Some(0)).0, vec![]);

        assert_matches!(
            open_pda_account_without_offset::<VerificationAccount>(&crate::id(), &payer, &invalid_pda_account),
            Err(_)
        );

        assert_matches!(
            open_pda_account_without_offset::<VerificationAccount>(&crate::id(), &payer, &pda_account),
            Ok(())
        );
    }

    #[test]
    fn test_open_pda_account() {
        test_account_info!(payer, 0);
        let seed = b"test";
        let seeds = vec![&seed[..], &seed[..]];
        let pda = Pubkey::find_program_address(&seeds, &crate::ID).0;
        let wrong_pda = VerificationAccount::find(Some(0)).0;

        account!(pda_account, wrong_pda, vec![]);
        assert_matches!(open_pda_account(&crate::id(), &payer, &pda_account, 1, &seeds), Err(_));

        account!(pda_account, pda, vec![]);
        assert_matches!(open_pda_account(&crate::id(), &payer, &pda_account, 1, &seeds), Ok(_));
    }

    #[test]
    #[ignore]
    fn test_create_pda_account() {
        panic!()
    }

    #[test]
    #[ignore]
    fn test_create_token_account_for_pda_authority() {
        panic!()
    }

    #[test]
    #[ignore]
    fn test_create_token_account() {
        panic!()
    }

    #[test]
    fn test_transfer_lamports_from_pda() {
        account!(pda, Pubkey::new_unique(), vec![]);
        account!(recipient, Pubkey::new_unique(), vec![]);

        unsafe {
            // Underflow
            let balance = pda.lamports();
            assert_matches!(transfer_lamports_from_pda(&pda, &recipient, balance + 1), Err(_));

            // Overflow
            assert_matches!(transfer_lamports_from_pda(&pda, &recipient, u64::MAX), Err(_));

            // Valid amount
            assert_matches!(transfer_lamports_from_pda(&pda, &recipient, balance), Ok(()));
            assert_eq!(pda.lamports(), 0);
            assert_eq!(recipient.lamports(), balance * 2);
        }
    }

    #[test]
    #[ignore]
    fn test_transfer_lamports_from_pda_checked() {
        panic!()
    }

    #[test]
    fn test_close_account() {
        account!(account, Pubkey::new_unique(), vec![]);
        account!(payer, Pubkey::new_unique(), vec![]);

        let start_balance = account.lamports();

        assert_matches!(close_account(&payer, &account), Ok(()));

        assert_eq!(account.lamports(), 0);
        assert_ne!(account.lamports(), start_balance);
        assert_eq!(payer.lamports(), start_balance * 2);
    }

    #[test]
    fn test_verify_token_account() {
        let token_account_pk0 = Pubkey::new_unique();
        let token_account_pk1 = Pubkey::new_unique();
        let mut data = vec![0; PoolAccount::SIZE];
        let mut pool = PoolAccount::new(&mut data).unwrap();
        pool.set_accounts(0, &ElusivOption::Some(token_account_pk0.to_bytes()));
        pool.set_accounts(1, &ElusivOption::Some(token_account_pk1.to_bytes()));

        let pool_pda = PoolAccount::find(None).0;
        account!(pool, pool_pda, data);
        account!(token_account0, token_account_pk0, vec![]);
        account!(token_account1, token_account_pk1, vec![]);

        assert_matches!(verify_pool(&pool, &pool, 0), Ok(()));

        assert_matches!(verify_pool(&pool, &token_account0, 1), Ok(_));
        assert_matches!(verify_pool(&pool, &token_account1, 1), Err(_));

        assert_matches!(verify_pool(&pool, &token_account1, 2), Ok(_));
        assert_matches!(verify_pool(&pool, &token_account0, 2), Err(_));
    }
}