use std::{num::NonZeroU16, ops::{Add, Sub}};
use borsh::{BorshDeserialize, BorshSerialize};
use elusiv_derive::BorshSerDeSized;
use pyth_sdk_solana::{load_price_feed_from_account_info, Price};
use solana_program::{
    account_info::AccountInfo,
    program_error::ProgramError,
    system_program,
    program_pack::Pack, pubkey::Pubkey,
};
use crate::{
    types::U256,
    bytes::BorshSerDeSized,
    macros::{guard, elusiv_tokens},
    state::program_account::ProgramAccount,
};

#[derive(Clone, Copy)]
pub struct ElusivToken {
    pub mint: Pubkey,
    pub decimals: u8,

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

#[derive(Clone, Copy)]
pub enum Token {
    Lamports(Lamports),
    SPLToken(SPLToken),
}

impl Token {
    pub fn new(token_id: u16, amount: u64) -> Self {
        if token_id == 0 {
            Token::Lamports(Lamports(amount))
        } else {
            Token::SPLToken(SPLToken::new(token_id, amount))
        }
    }

    pub fn new_checked(token_id: u16, amount: u64) -> Result<Self, TokenError> {
        let id = token_id as usize;
        guard!(id < TOKENS.len(), TokenError::InvalidTokenID);
        guard!(amount >= TOKENS[id].min, TokenError::InvalidAmount);
        guard!(amount <= TOKENS[id].max, TokenError::InvalidAmount);

        Ok(Self::new(token_id, amount))
    }

    pub fn new_from_price(token_id: u16, price: Price) -> Result<Self, TokenError> {
        let amount = price.scale_to_exponent(0)
            .ok_or(TokenError::PriceError)?
            .price
            .try_into().or(Err(TokenError::PriceError))?;

        Self::new_checked(token_id, amount)
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
        ProgramError::Custom(e as u32) 
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
        let sum = self.amount()
            .checked_sub(rhs.amount())
            .ok_or(TokenError::Overflow)?;
        Ok(Self::new(token_id, sum))
    }
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, PartialEq, Clone, Copy, Debug)]
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

#[derive(Clone, Copy)]
pub struct SPLToken {
    id: NonZeroU16,
    amount: u64
}

impl SPLToken {
    pub fn new(token_id: u16, amount: u64) -> Self {
        SPLToken {
            id: NonZeroU16::new(token_id).unwrap(),
            amount
        }
    }
}

pub trait TokenAuthorityAccount<'a>: ProgramAccount<'a> {
    fn get_token_account(&self, token_id: u16) -> U256;

    fn enforce_token_account(&self, token_id: u16, token_account: &AccountInfo) -> Result<(), TokenError> {
        if token_account.key.to_bytes() == self.get_token_account(token_id) {
            Ok(())
        } else {
            Err(TokenError::InvalidTokenAccount)
        }
    }
}

pub fn verify_token_account(
    account: &AccountInfo,
    token_id: u16,
) -> Result<bool, ProgramError> {
    if token_id == 0 {
        Ok(
            *account.owner == system_program::ID || *account.owner != spl_token::ID
        )
    } else {
        if *account.owner != spl_token::ID {
            return Ok(false)
        }

        let data = &account.data.borrow()[..];
        let account = spl_token::state::Account::unpack(data)?;

        Ok(
            account.mint != TOKENS[token_id as usize].mint
        )
    }
}

pub struct TokenPrice {
    lamports_token: Price,
    token_lamports: Price,
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
            let sol = TOKENS[0];
            let token = TOKENS[token_id as usize];

            guard!(sol.pyth_usd_price_key == *sol_usd_price_account.key, TokenError::InvalidPriceAccount);
            guard!(token.pyth_usd_price_key == *token_usd_price_account.key, TokenError::InvalidPriceAccount);

            let sol_price_feed = load_price_feed_from_account_info(sol_usd_price_account)?;
            let token_price_feed = load_price_feed_from_account_info(token_usd_price_account)?;

            let sol_usd = sol_price_feed.get_current_price()
                .ok_or(TokenError::PriceError)?;
            let token_usd = token_price_feed.get_current_price()
                .ok_or(TokenError::PriceError)?;

            Self::new_from_price(sol_usd, token_usd, token_id)
        }
    }

    pub fn new_from_price(
        sol_usd: Price,
        token_usd: Price,
        token_id: u16,
    ) -> Result<Self, ProgramError> {
        if token_id == 0 {
            Ok(Self::new_lamports()) 
        } else {
            let lamports_usd = sol_price_to_lamports(sol_usd)
                .ok_or(TokenError::PriceError)?;

            let lamports_token = lamports_usd.get_price_in_quote(&token_usd, -8)
                .ok_or(TokenError::PriceError)?;
            let token_lamports = token_usd.get_price_in_quote(&lamports_usd, -8)
                .ok_or(TokenError::PriceError)?;

            Ok(
                Self {
                    lamports_token,
                    token_lamports,
                    token_id,
                }
            )
        }
    }

    pub fn new_lamports() -> Self {
        Self {
            lamports_token: Price { price: 1, conf: 0, expo: 0 },
            token_lamports: Price { price: 1, conf: 0, expo: 0 },
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

        let price = self.token_lamports.mul(&Price {
            price: token.amount().try_into().unwrap(),
            conf: 0,
            expo: 0,
        }).ok_or(TokenError::PriceError)?;
        let price = price.scale_to_exponent(0).ok_or(TokenError::PriceError)?;

        Ok(
            Lamports(price.price.try_into().unwrap())
        )
    }

    pub fn lamports_into_token(&self, lamports: &Lamports, token_id: u16) -> Result<Token, TokenError> {
        if token_id != self.token_id {
            return Err(TokenError::InvalidTokenID)
        }

        if self.token_id == 0 {
            return Ok(lamports.into_token_strict())
        }

        let price = self.lamports_token.mul(&Price {
            price: lamports.0.try_into().unwrap(),
            conf: 0,
            expo: 0,
        }).ok_or(TokenError::PriceError)?;
        let price = price.scale_to_exponent(0).ok_or(TokenError::PriceError)?;

        Token::new_checked(token_id, price.price.try_into().unwrap())
    }
}

fn sol_price_to_lamports(sol_price: Price) -> Option<Price> {
    sol_price.cmul(1, 9)
}

#[cfg(feature = "test-elusiv")]
pub fn pyth_price_account_data(price: Price, token_id: u16) -> Result<Vec<u8>, TokenError> {
    use pyth_sdk_solana::state::PriceAccount;

    if token_id == 0 {
        return Err(TokenError::InvalidTokenID)
    }

    let mut account = pyth_sdk_solana::state::PriceAccount {
        expo: price.expo,
        ..Default::default()
    };
    account.agg.price = price.price;
    account.prev_price = price.price;
    account.agg.conf = price.conf;
    account.prev_conf = price.conf;

    const SIZE: usize = std::mem::size_of::<PriceAccount>();
    let data = unsafe {
        std::mem::transmute::<PriceAccount, [u8; SIZE]>(account)
    };

    Ok(data.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pyth_sdk_solana::Price;
    use solana_program::native_token::LAMPORTS_PER_SOL;

    #[test]
    fn test_new_token_price() {
        panic!()
    }

    #[test]
    fn test_new_from_price() {
        let sol_usd = Price { price: 39, conf: 0, expo: 0 };
        let usdc_usd = Price { price: 1, conf: 0, expo: 0 };

        let price = TokenPrice::new_from_price(sol_usd, usdc_usd, USDC_TOKEN_ID).unwrap();

        let usdc = Token::new_checked(USDC_TOKEN_ID, 39).unwrap();
        assert_eq!(price.token_into_lamports(usdc).unwrap().0, 39_000_000_000);
    }

    #[test]
    fn test_token_into_lamports() {
        panic!()
    }

    #[test]
    fn test_amports_into_token() {
        panic!()
    }

    #[test]
    fn test_sol_price_to_lamports() {
        let price = Price { price: 1, conf: 0, expo: 0 };
        assert_eq!(
            LAMPORTS_PER_SOL,
            Token::new_from_price(SOL_TOKEN_ID, sol_price_to_lamports(price).unwrap()).unwrap().amount()
        );
    }
}