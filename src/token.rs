use std::{num::NonZeroU16, ops::{Add, Sub}};
use borsh::{BorshDeserialize, BorshSerialize};
use elusiv_derive::BorshSerDeSized;
use pyth_sdk_solana::{load_price_feed_from_account_info, Price};
use solana_program::{
    account_info::AccountInfo,
    program_error::ProgramError,
    program_pack::Pack, pubkey::Pubkey,
};
use spl_associated_token_account::get_associated_token_address;
use crate::{
    types::U256,
    bytes::BorshSerDeSized,
    macros::{guard, elusiv_tokens},
    state::program_account::PDAAccount,
};

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ElusivToken {
    pub mint: Pubkey,
    pub decimals: u8,
    pub price_base_exp: u8,

    /// Key of the Pyth price account
    pub pyth_usd_price_key: Pubkey,

    /// Inclusive minimum
    pub min: u64,

    /// Inclusive maximum
    pub max: u64,
}

#[cfg(not(feature = "devnet"))]
elusiv_tokens!(mainnet);

#[cfg(feature = "devnet")]
elusiv_tokens!(devnet);

pub fn elusiv_token(token_id: u16) -> Result<ElusivToken, TokenError> {
    let token_id = token_id as usize;
    if token_id > SPL_TOKEN_COUNT {
        Err(TokenError::InvalidTokenID)
    } else {
        Ok(TOKENS[token_id])
    }
}

pub const SPL_TOKEN_COUNT: usize = TOKENS.len() - 1;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Token {
    Lamports(Lamports),
    SPLToken(SPLToken),
}

impl Token {
    pub fn new(token_id: u16, amount: u64) -> Self {
        if token_id == 0 {
            Token::Lamports(Lamports(amount))
        } else {
            Token::SPLToken(SPLToken::new(token_id, amount).unwrap())
        }
    }

    pub fn new_checked(token_id: u16, amount: u64) -> Result<Self, TokenError> {
        let id = token_id as usize;
        guard!(id < TOKENS.len(), TokenError::InvalidTokenID);
        guard!(amount >= TOKENS[id].min, TokenError::InvalidAmount);
        guard!(amount <= TOKENS[id].max, TokenError::InvalidAmount);

        Ok(Self::new(token_id, amount))
    }

    pub fn new_from_price(token_id: u16, price: Price, check_amount: bool) -> Result<Self, TokenError> {
        let target_expo = if token_id == 0 { 0 } else { -(elusiv_token(token_id)?.decimals as i32) };
        let amount = price.scale_to_exponent(target_expo)
            .ok_or(TokenError::PriceError)?
            .price
            .try_into().or(Err(TokenError::PriceError))?;

        if check_amount {
            Self::new_checked(token_id, amount)
        } else {
            Ok(Self::new(token_id, amount))
        }
    }

    fn enforce_token_equality(&self, other: &Self) -> Result<u16, TokenError> {
        let token_id = self.token_id();
        guard!(token_id == other.token_id(), TokenError::MismatchedTokenID);

        Ok(token_id)
    }

    pub fn token_id(&self) -> u16 {
        match self {
            Token::Lamports(_) => 0,
            Token::SPLToken(SPLToken { id, .. }) => id.get()
        }
    }

    pub fn amount(&self) -> u64 {
        match self {
            Token::Lamports(Lamports(amount)) => *amount,
            Token::SPLToken(SPLToken { amount, .. }) => *amount
        }
    }

    pub fn into_lamports(&self) -> Result<Lamports, TokenError> {
        match self {
            Token::Lamports(lamports) => Ok(*lamports),
            _ => Err(TokenError::InvalidTokenID)
        }
    }
}

#[derive(Debug)]
pub enum TokenError {
    MismatchedTokenID,
    InvalidTokenID,
    InvalidAmount,

    InvalidTokenAccount,
    InvalidPriceAccount,
    PriceError,

    Underflow,
    Overflow,
}

impl From<TokenError> for ProgramError {
    fn from(e: TokenError) -> Self {
        ProgramError::Custom(e as u32 + 100) 
    }
}

impl Add for Token {
    type Output = Result<Self, TokenError>;

    fn add(self, rhs: Self) -> Self::Output {
        let token_id = self.enforce_token_equality(&rhs)?;
        let sum = self.amount()
            .checked_add(rhs.amount())
            .ok_or(TokenError::Overflow)?;
        Ok(Self::new(token_id, sum))
    }
}

impl Sub for Token {
    type Output = Result<Self, TokenError>;

    fn sub(self, rhs: Self) -> Self::Output {
        let token_id = self.enforce_token_equality(&rhs)?;
        let dif = self.amount()
            .checked_sub(rhs.amount())
            .ok_or(TokenError::Underflow)?;
        Ok(Self::new(token_id, dif))
    }
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, PartialEq, Clone, Copy, Debug, Default)]
pub struct Lamports(pub u64);

impl Lamports {
    pub fn into_token(
        &self,
        price: &TokenPrice,
        token_id: u16,
    ) -> Result<Token, TokenError> {
        price.lamports_into_token(self, token_id)
    }

    pub fn into_token_strict(&self) -> Token {
        Token::Lamports(*self)
    }
}

impl Add for Lamports {
    type Output = Result<Self, TokenError>;

    fn add(self, rhs: Self) -> Self::Output {
        let sum = self.0.checked_add(rhs.0).ok_or(TokenError::Overflow)?;
        Ok(Lamports(sum))
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct SPLToken {
    pub id: NonZeroU16,
    pub amount: u64
}

impl SPLToken {
    pub fn new(token_id: u16, amount: u64) -> Result<Self, TokenError> {
        Ok(
            SPLToken {
                id: NonZeroU16::new(token_id).ok_or(TokenError::InvalidTokenID)?,
                amount
            }
        )
    }
}

pub trait TokenAuthorityAccount: PDAAccount {
    #[allow(clippy::missing_safety_doc)]
    unsafe fn get_token_account_unchecked(&self, token_id: u16) -> Option<U256>;

    #[allow(clippy::missing_safety_doc)]
    unsafe fn set_token_account_unchecked(&mut self, token_id: u16, key: &Pubkey);

    fn enforce_token_account(&self, token_id: u16, token_account: &AccountInfo) -> Result<(), TokenError> {
        if token_account.key.to_bytes() == self.get_token_account(token_id).ok_or(TokenError::InvalidTokenAccount)? {
            Ok(())
        } else {
            Err(TokenError::InvalidTokenAccount)
        }
    }

    fn get_token_account(&self, token_id: u16) -> Option<U256> {
        if token_id == 0 || token_id as usize > TOKENS.len() {
            None
        } else {
            unsafe { self.get_token_account_unchecked(token_id) }
        }
    }

    fn try_set_token_account(&mut self, token_id: u16, key: &Pubkey) -> Result<(), TokenError> {
        if token_id == 0 || self.get_token_account(token_id).is_some() {
            return Err(TokenError::InvalidTokenID)
        }
        unsafe { self.set_token_account_unchecked(token_id, key) }
        Ok(())
    }
}

/// Ensures that a given account is able to receive the specified token
pub fn verify_token_account(
    account: &AccountInfo,
    token_id: u16,
) -> Result<bool, ProgramError> {
    if token_id == 0 {
        Ok(*account.owner != spl_token::ID)
    } else {
        if *account.owner != spl_token::ID {
            return Ok(false)
        }

        let data = &account.data.borrow()[..];
        let account = spl_token::state::Account::unpack(data)?;

        Ok(account.mint == elusiv_token(token_id)?.mint)
    }
}

pub fn verify_associated_token_account(
    wallet_address: &Pubkey,
    token_account_address: &Pubkey,
    token_id: u16,
) -> Result<bool, ProgramError> {
    if token_id == 0 {
        Ok(*wallet_address == *token_account_address)
    } else {
        let expected = get_associated_token_address(
            wallet_address,
            &elusiv_token(token_id)?.mint,
        );

        Ok(*token_account_address == expected)
    }
}

pub struct TokenPrice {
    lamports_usd: Price,
    token_usd: Price,
    token_id: u16,
}

impl TokenPrice {
    pub fn new(
        sol_usd_price_account: &AccountInfo,
        token_usd_price_account: &AccountInfo,
        token_id: u16,
    ) -> Result<Self, ProgramError> {
        if token_id == 0 {
            Ok(Self::new_lamports()) 
        } else {
            let lamports = TOKENS[0];
            let token = TOKENS[token_id as usize];

            guard!(lamports.pyth_usd_price_key == *sol_usd_price_account.key, TokenError::InvalidPriceAccount);
            guard!(token.pyth_usd_price_key == *token_usd_price_account.key, TokenError::InvalidPriceAccount);

            let lamports_usd = Self::load_token_usd_price(sol_usd_price_account, 0)?;
            let token_usd = Self::load_token_usd_price(token_usd_price_account, token_id)?;

            Ok(Self::new_from_price(lamports_usd, token_usd, token_id))
        }
    }

    fn load_token_usd_price(
        token_usd_price_account: &AccountInfo,
        token_id: u16,
    ) -> Result<Price, TokenError> {
        let price_feed = load_price_feed_from_account_info(token_usd_price_account)
            .or(Err(TokenError::PriceError))?;

        let base_price = price_feed.get_current_price()
            .ok_or(TokenError::PriceError)?;
        let price = base_price.cmul(1, -(elusiv_token(token_id)?.price_base_exp as i32))
            .ok_or(TokenError::PriceError)?;

        Ok(price)
    }

    pub fn new_from_price(
        lamports_usd: Price,
        token_usd: Price,
        token_id: u16,
    ) -> Self {
        if token_id == 0 {
            Self::new_lamports()
        } else {
            Self { lamports_usd, token_usd, token_id }
        }
    }

    pub fn new_from_sol_price(
        sol_usd: Price,
        token_usd: Price,
        token_id: u16,
    ) -> Result<Self, TokenError> {
        if token_id == 0 {
            Ok(Self::new_lamports())
        } else {
            let lamports_usd = sol_usd.cmul(1, -(elusiv_token(0)?.price_base_exp as i32))
                .ok_or(TokenError::PriceError)?;

            Ok(Self { lamports_usd, token_usd, token_id })
        }
    }

    pub fn new_lamports() -> Self {
        Self {
            lamports_usd: Price { price: 1, conf: 0, expo: 0 },
            token_usd: Price { price: 1, conf: 0, expo: 0 },
            token_id: 0,
        }
    }

    pub fn token_into_lamports(&self, token: Token) -> Result<Lamports, TokenError> {
        if token.token_id() != self.token_id {
            return Err(TokenError::InvalidTokenID)
        }

        if self.token_id == 0 {
            return Ok(Lamports(token.amount()))
        }

        let usd = self.token_usd.mul(
            &Price {
                price: token.amount().try_into().unwrap(),
                conf: 0,
                expo: -(elusiv_token(self.token_id)?.decimals as i32),
            }
        ).ok_or(TokenError::PriceError)?;
        let price = usd.get_price_in_quote(&self.lamports_usd, 0).ok_or(TokenError::PriceError)?;
        Token::new_from_price(0, price, false)?.into_lamports()
    }

    pub fn lamports_into_token(&self, lamports: &Lamports, token_id: u16) -> Result<Token, TokenError> {
        if token_id != self.token_id {
            return Err(TokenError::InvalidTokenID)
        }

        if self.token_id == 0 {
            return Ok(lamports.into_token_strict())
        }

        let usd = self.lamports_usd.mul(
            &Price {
                price: lamports.0.try_into().unwrap(),
                conf: 0,
                expo: 0,
            }
        ).ok_or(TokenError::PriceError)?;
        let price = usd.get_price_in_quote(&self.token_usd, -(elusiv_token(self.token_id)?.decimals as i32)).ok_or(TokenError::PriceError)?;
        Token::new_from_price(token_id, price, false)
    }
}

#[cfg(feature = "test-elusiv")]
pub fn pyth_price_account_data(price: &Price) -> Result<Vec<u8>, TokenError> {
    use bytemuck::bytes_of;
    use pyth_sdk_solana::{state::{MAGIC, VERSION_2, AccountType}, PriceStatus};

    let mut account = pyth_sdk_solana::state::PriceAccount {
        magic: MAGIC,
        ver: VERSION_2,
        atype: AccountType::Price as u32,
        expo: price.expo,
        ..Default::default()
    };
    account.agg.price = price.price;
    account.prev_price = price.price;
    account.agg.conf = price.conf;
    account.prev_conf = price.conf;
    account.agg.status = PriceStatus::Trading;

    Ok(bytes_of(&account).to_vec())
}

#[cfg(feature = "test-elusiv")]
pub fn spl_token_account_data(token_id: u16) -> Vec<u8> {
    let account = spl_token::state::Account {
        mint: elusiv_token(token_id).unwrap().mint,
        state: spl_token::state::AccountState::Initialized,
        ..Default::default()
    };
    let mut data = vec![0; spl_token::state::Account::LEN];
    spl_token::state::Account::pack(account, &mut data[..]).unwrap();
    data
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::macros::{account, pyth_price_account_info};
    use assert_matches::assert_matches;
    use pyth_sdk_solana::Price;
    use solana_program::native_token::LAMPORTS_PER_SOL;

    macro_rules! test_token_id {
        ($id: ident, $id_num: literal, $fn: ident) => {
            assert_eq!($fn(), TOKENS[$id_num as usize]);
            assert_eq!($fn(), elusiv_token($id_num).unwrap());
            assert_eq!($id, $id_num);
        };
    }

    #[test]
    fn test_token_ids() {
        test_token_id!(LAMPORTS_TOKEN_ID, 0, lamports_token);
        test_token_id!(USDC_TOKEN_ID, 1, usdc_token);
        test_token_id!(USDT_TOKEN_ID, 2, usdt_token);
    }

    #[test]
    #[allow(unused_variables)]
    fn test_token_new() {
        assert_matches!(Token::new(0, 123), Token::Lamports(Lamports(123)));
        let id = NonZeroU16::new(99).unwrap();
        assert_matches!(Token::new(99, 456), Token::SPLToken(SPLToken { amount: 456, id }));
    }

    #[test]
    #[allow(unused_variables)]
    fn test_token_new_checked() {
        assert_matches!(Token::new_checked(TOKENS.len() as u16, 1_000_000), Err(TokenError::InvalidTokenID));

        let min = lamports_token().min;
        let max = lamports_token().max;
        assert_matches!(Token::new_checked(0, max + 1), Err(TokenError::InvalidAmount));
        assert_matches!(Token::new_checked(0, min - 1), Err(TokenError::InvalidAmount));

        assert_matches!(Token::new_checked(0, lamports_token().max), Ok(Token::Lamports(Lamports(max))));
        assert_matches!(Token::new_checked(0, lamports_token().min), Ok(Token::Lamports(Lamports(min))));
    }

    #[test]
    fn test_token_new_from_price() {
        let price = Price { price: 123456, expo: -30, conf: 100 };
        assert_matches!(Token::new_from_price(0, price, true), Err(TokenError::InvalidAmount));

        let price = Price { price: 123456789, expo: -2, conf: 100 };
        assert_matches!(Token::new_from_price(0, price, true), Ok(Token::Lamports(Lamports(1234567))));
    }

    #[test]
    fn test_enforce_token_equality() {
        let a = Token::new(0, 1_000_000);
        let b = Token::new(1, 1_000_000);
        assert_matches!(a.enforce_token_equality(&b), Err(TokenError::MismatchedTokenID));

        let a = Token::new(1, 1_000_000);
        assert_matches!(a.enforce_token_equality(&b), Ok(1));
    }

    #[test]
    fn test_token_id() {
        assert_eq!(Token::new(0, 10).token_id(), 0);
        assert_eq!(Token::new(1, 10).token_id(), 1);
        assert_eq!(Token::new(2, 10).token_id(), 2);
    }

    #[test]
    fn test_token_amount() {
        assert_eq!(Token::new(0, 99_000_000_000).amount(), 99_000_000_000);
        assert_eq!(Token::new(1, 123_456).amount(), 123_456);
    }

    #[test]
    fn test_into_lamports() {
        assert_matches!(Token::new(0, 10).into_lamports(), Ok(Lamports(10)));
        assert_matches!(Token::new(1, 10).into_lamports(), Err(TokenError::InvalidTokenID));
    }

    #[test]
    fn test_add_tokens() {
        assert_matches!(Token::new(0, 10).add(Token::new(1, 10)), Err(TokenError::MismatchedTokenID));
        assert_matches!(Token::new(0, u64::MAX).add(Token::new(0, 1)), Err(TokenError::Overflow));
        assert_matches!(Token::new(0, 123).add(Token::new(0, 1_000)), Ok(Token::Lamports(Lamports(1_123))));
    }

    #[test]
    fn test_sub_tokens() {
        assert_matches!(Token::new(0, 10).sub(Token::new(1, 10)), Err(TokenError::MismatchedTokenID));
        assert_matches!(Token::new(0, 0).sub(Token::new(0, 1)), Err(TokenError::Underflow));
        assert_matches!(Token::new(0, 123).sub(Token::new(0, 23)), Ok(Token::Lamports(Lamports(100))));
    }

    #[test]
    fn test_lamports_into_token_strict() {
        assert_matches!(Lamports(123).into_token_strict(), Token::Lamports(Lamports(123)));
    }

    #[test]
    fn test_add_lamports() {
        assert_matches!(Lamports(u64::MAX).add(Lamports(1)), Err(TokenError::Overflow));
        assert_matches!(Lamports(100).add(Lamports(23)), Ok(Lamports(123)));
    }

    #[test]
    #[allow(unused_variables)]
    fn test_spl_token_new() {
        assert_matches!(SPLToken::new(0, 10), Err(TokenError::InvalidTokenID));
        let id = NonZeroU16::new(1).unwrap();
        assert_matches!(SPLToken::new(1, 10), Ok(SPLToken { id, amount: 10 }));
    }

    struct TestTokenAuthorityAccount {
        token_accounts: [Option<U256>; SPL_TOKEN_COUNT],
    }

    impl PDAAccount for TestTokenAuthorityAccount {
        const SEED: &'static [u8] = b"TEST";
    }

    impl TokenAuthorityAccount for TestTokenAuthorityAccount {
        unsafe fn get_token_account_unchecked(&self, token_id: u16) -> Option<U256> {
            self.token_accounts[token_id as usize - 1]
        }

        unsafe fn set_token_account_unchecked(&mut self, token_id: u16, key: &Pubkey) {
            self.token_accounts[token_id as usize - 1] = Some(key.to_bytes())
        }
    }

    #[test]
    #[allow(unused_variables)]
    fn test_token_authority_account() {
        let mut account = TestTokenAuthorityAccount {
            token_accounts: [None; SPL_TOKEN_COUNT]
        };
        let pk = Pubkey::new_unique();
        assert_matches!(account.get_token_account(1), None);
        assert_matches!(account.try_set_token_account(1, &pk), Ok(()));
        assert_matches!(account.get_token_account(1), Some(pk));
        assert_matches!(account.try_set_token_account(1, &pk), Err(_));
        assert_matches!(account.try_set_token_account(0, &pk), Err(_));
    }

    #[test]
    fn test_verify_token_account() {
        account!(sol_account, Pubkey::new_unique(), vec![]);

        assert!(verify_token_account(&sol_account, 0).unwrap());
        assert!(!verify_token_account(&sol_account, 1).unwrap());

        let data = spl_token_account_data(USDC_TOKEN_ID);
        account!(usdc_account, Pubkey::new_unique(), data.clone());
        assert!(!verify_token_account(&usdc_account, 1).unwrap());

        account!(usdc_account, Pubkey::new_unique(), data, spl_token::id());
        assert!(verify_token_account(&usdc_account, 1).unwrap());
        assert!(!verify_token_account(&usdc_account, 0).unwrap());
        assert!(!verify_token_account(&usdc_account, 2).unwrap());
    }

    #[test]
    fn test_token_price_new() {
        let sol_usd = Price { price: 39, conf: 1, expo: 0 };    // 1 SOL = 39 USD +- 1 USD
        pyth_price_account_info!(sol_usd_account, LAMPORTS_TOKEN_ID, sol_usd);

        let usdc_usd = Price { price: 1, conf: 1, expo: 0 };    // 1 USDC = 1 USD
        pyth_price_account_info!(usdc_usd_account, USDC_TOKEN_ID, usdc_usd);

        let price = TokenPrice::new(
            &sol_usd_account,
            &usdc_usd_account,
            USDC_TOKEN_ID,
        ).unwrap();

        assert_eq!(price.lamports_usd, Price { price: sol_usd.price, conf: sol_usd.conf, expo: -9 });
        assert_eq!(price.token_usd, usdc_usd);
    }

    #[test]
    fn test_load_token_usd_price() {
        let sol_usd = Price { price: 39, conf: 1, expo: 0 };    // 1 SOL = 39 USD +- 1 USD
        pyth_price_account_info!(sol_usd_account, LAMPORTS_TOKEN_ID, sol_usd);
        let lamports_usd = TokenPrice::load_token_usd_price(&sol_usd_account, LAMPORTS_TOKEN_ID).unwrap();
        assert_eq!(lamports_usd.price, sol_usd.price);
        assert_eq!(lamports_usd.conf, sol_usd.conf);
        assert_eq!(lamports_usd.expo, -9);

        let reduced = lamports_usd.scale_to_exponent(0).unwrap();
        assert_eq!(reduced.price, 0);
        assert_eq!(reduced.conf, 0);
    }

    #[test]
    fn test_token_price_new_from_price() {
        let lamports_usd = Price { price: 39, conf: 123, expo: -9 };
        let token_usd = Price { price: 1, conf: 45, expo: 0 };

        let price = TokenPrice::new_from_price(lamports_usd, token_usd, USDC_TOKEN_ID);

        assert_eq!(price.lamports_usd, lamports_usd);
        assert_eq!(price.token_usd, token_usd);
        assert_eq!(price.token_id, USDC_TOKEN_ID);
    }

    #[test]
    fn test_new_from_sol_price() {
        let sol_usd = Price { price: 390, conf: 123, expo: -1 };
        let token_usd = Price { price: 99, conf: 123, expo: -2 };

        let price = TokenPrice::new_from_sol_price(sol_usd, token_usd, USDT_TOKEN_ID).unwrap();

        let lamports_usd = Price { price: 390, conf: 123, expo: -10 };
        assert_eq!(price.lamports_usd, lamports_usd);
        assert_eq!(price.token_usd, token_usd);
        assert_eq!(price.token_id, USDT_TOKEN_ID);
    }

    #[test]
    fn test_token_price_new_lamports() {
        let price = TokenPrice::new_lamports();
        assert_matches!(price.token_into_lamports(Token::Lamports(Lamports(123))), Ok(Lamports(123)));
    }

    #[test]
    fn test_token_into_lamports() {
        // 1 LAMPORT = 39 * 10^{-9} USD
        let lamports_usd = Price { price: 39, conf: 0, expo: -9 };
        // 1 USDC = 0.5 USD
        let token_usd = Price { price: 500_000, conf: 0, expo: -6 };
        let price = TokenPrice::new_from_price(lamports_usd, token_usd, USDC_TOKEN_ID);

        // 1 USD = 1 / 39 * 10^{-9} LAMPORTS
        // 1 USDC = 0.5 * 1 / (39 * 10^{-9}) LAMPORTS (https://www.wolframalpha.com/input?i=0.5+*+1+%2F+%2839+*+power+%2810%2C+-9%29%29)
        assert_eq!(12820512, price.token_into_lamports(Token::new(USDC_TOKEN_ID, 1_000_000)).unwrap().0);

        // 99 USDC = 0.5 * 99 * 1 / (39 * 10^{-9}) LAMPORTS (https://www.wolframalpha.com/input?i=0.5+*+99+*+1+%2F+%2839+*+power+%2810%2C+-9%29%29)
        assert_eq!(1269230769, price.token_into_lamports(Token::new(USDC_TOKEN_ID, 99_000_000)).unwrap().0);
    }

    #[test]
    fn test_lamports_into_token() {
        // 1 LAMPORT = 39 * 10^{-9} USD
        let lamports_usd = Price { price: 39, conf: 0, expo: -9 };
        // 1 USDC = 0.5 USD
        let token_usd = Price { price: 500_000, conf: 0, expo: -6 };
        let price = TokenPrice::new_from_price(lamports_usd, token_usd, USDC_TOKEN_ID);

        // 1 LAMPORT = 39 * 10^{-9} * 2 USDC = 0.000_000_078 USDC (https://www.wolframalpha.com/input?i=2+*+39+*+power+%2810%2C+-9%29)
        assert_eq!(0, price.lamports_into_token(&Lamports(1), USDC_TOKEN_ID).unwrap().amount());

        // 1_000 LAMPORTS = 1_000 * 39 * 10^{-9} * 2 USDC = 0.000_078 USDC (https://www.wolframalpha.com/input?i=1000+*+2+*+39+*+power+%2810%2C+-9%29)
        assert_eq!(78, price.lamports_into_token(&Lamports(1_000), USDC_TOKEN_ID).unwrap().amount());

        // 99 SOL = 99 * 10^9 LAMPORTS = 99 * 10^9 * 39 * 10^{-9} * 2 USDC = 99 * 39 * 2 USDC
        assert_eq!(99 * 39 * 2 * 1_000_000, price.lamports_into_token(&Lamports(99 * LAMPORTS_PER_SOL), USDC_TOKEN_ID).unwrap().amount());
    }

    #[test]
    fn test_pyth_price_account_data() {
        let price = Price { price: 123, conf: 456, expo: 7 };
        pyth_price_account_info!(sol_usd_account, LAMPORTS_TOKEN_ID, price);
        let price_feed = load_price_feed_from_account_info(&sol_usd_account).unwrap();
        assert_eq!(price, price_feed.get_current_price().unwrap());
        assert_eq!(*sol_usd_account.key, TOKENS[LAMPORTS_TOKEN_ID as usize].pyth_usd_price_key);
    }
}