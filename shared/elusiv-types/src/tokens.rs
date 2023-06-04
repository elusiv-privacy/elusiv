use crate as elusiv_types;
use borsh::{BorshDeserialize, BorshSerialize};
use elusiv_derive::BorshSerDeSized;
use solana_program::{
    account_info::AccountInfo, program_error::ProgramError, program_pack::Pack, pubkey::Pubkey,
};
use spl_associated_token_account::get_associated_token_address;
use std::{
    num::NonZeroU16,
    ops::{Add, Sub},
};

pub use pyth_sdk_solana::{load_price_feed_from_account_info, Price};

#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(any(test, feature = "elusiv-client"), derive(Debug))]
pub struct ElusivToken {
    #[cfg(feature = "elusiv-client")]
    pub ident: &'static str,

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

elusiv_proc_macros::elusiv_tokens!();

pub fn elusiv_token(token_id: TokenID) -> Result<ElusivToken, TokenError> {
    let token_id = token_id as usize;
    if token_id > SPL_TOKEN_COUNT {
        Err(TokenError::InvalidTokenID)
    } else {
        Ok(TOKENS[token_id])
    }
}

pub type TokenID = u16;

pub const SPL_TOKEN_COUNT: usize = TOKENS.len() - 1;

#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(any(test, feature = "elusiv-client"), derive(Debug))]
pub enum Token {
    Lamports(Lamports),
    SPLToken(SPLToken),
}

impl Token {
    pub fn new(token_id: TokenID, amount: u64) -> Self {
        if token_id == 0 {
            Token::Lamports(Lamports(amount))
        } else {
            Token::SPLToken(SPLToken::new(token_id, amount).unwrap())
        }
    }

    pub fn new_checked(token_id: TokenID, amount: u64) -> Result<Self, TokenError> {
        let id = token_id as usize;

        if id >= TOKENS.len() {
            return Err(TokenError::InvalidTokenID);
        }

        if amount < TOKENS[id].min || amount > TOKENS[id].max {
            return Err(TokenError::InvalidAmount);
        }

        Ok(Self::new(token_id, amount))
    }

    pub fn new_from_price(
        token_id: u16,
        price: Price,
        check_amount: bool,
    ) -> Result<Self, TokenError> {
        let target_expo = if token_id == 0 {
            0
        } else {
            -(elusiv_token(token_id)?.decimals as i32)
        };
        let amount = price
            .scale_to_exponent(target_expo)
            .ok_or(TokenError::PriceError)?
            .price
            .try_into()
            .or(Err(TokenError::PriceError))?;

        if check_amount {
            Self::new_checked(token_id, amount)
        } else {
            Ok(Self::new(token_id, amount))
        }
    }

    pub fn enforce_token_equality(&self, other: &Self) -> Result<TokenID, TokenError> {
        let token_id = self.token_id();

        if token_id != other.token_id() {
            return Err(TokenError::MismatchedTokenID);
        }

        Ok(token_id)
    }

    pub fn token_id(&self) -> TokenID {
        match self {
            Token::Lamports(_) => 0,
            Token::SPLToken(SPLToken { id, .. }) => id.get(),
        }
    }

    pub fn amount(&self) -> u64 {
        match self {
            Token::Lamports(Lamports(amount)) => *amount,
            Token::SPLToken(SPLToken { amount, .. }) => *amount,
        }
    }

    pub fn into_lamports(&self) -> Result<Lamports, TokenError> {
        match self {
            Token::Lamports(lamports) => Ok(*lamports),
            _ => Err(TokenError::InvalidTokenID),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
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
        let sum = self
            .amount()
            .checked_add(rhs.amount())
            .ok_or(TokenError::Overflow)?;
        Ok(Self::new(token_id, sum))
    }
}

impl Sub for Token {
    type Output = Result<Self, TokenError>;

    fn sub(self, rhs: Self) -> Self::Output {
        let token_id = self.enforce_token_equality(&rhs)?;
        let dif = self
            .amount()
            .checked_sub(rhs.amount())
            .ok_or(TokenError::Underflow)?;
        Ok(Self::new(token_id, dif))
    }
}

#[derive(
    BorshDeserialize, BorshSerialize, BorshSerDeSized, PartialEq, Eq, Clone, Copy, Default,
)]
#[cfg_attr(feature = "elusiv-client", derive(Debug))]
pub struct Lamports(pub u64);

impl Lamports {
    pub fn into_token(&self, price: &TokenPrice, token_id: TokenID) -> Result<Token, TokenError> {
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

#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "elusiv-client", derive(Debug))]
pub struct SPLToken {
    pub id: NonZeroU16,
    pub amount: u64,
}

impl SPLToken {
    pub fn new(token_id: TokenID, amount: u64) -> Result<Self, TokenError> {
        Ok(SPLToken {
            id: NonZeroU16::new(token_id).ok_or(TokenError::InvalidTokenID)?,
            amount,
        })
    }
}

/// Ensures that a given account is able to receive the specified token
pub fn verify_token_account(
    account: &AccountInfo,
    token_id: TokenID,
) -> Result<bool, ProgramError> {
    if token_id == 0 {
        Ok(*account.owner != spl_token::ID)
    } else {
        if *account.owner != spl_token::ID {
            return Ok(false);
        }

        let data = &account.data.borrow()[..];
        let account = spl_token::state::Account::unpack(data)?;

        Ok(account.mint == elusiv_token(token_id)?.mint)
    }
}

/// Verifies an associated-token-account for a given token-id
pub fn verify_associated_token_account(
    wallet_address: &Pubkey,
    token_account_address: &Pubkey,
    token_id: TokenID,
) -> Result<bool, ProgramError> {
    if token_id == 0 {
        Ok(*wallet_address == *token_account_address)
    } else {
        let expected = get_associated_token_address(wallet_address, &elusiv_token(token_id)?.mint);

        Ok(*token_account_address == expected)
    }
}

pub struct TokenPrice {
    pub lamports_usd: Price,
    pub token_usd: Price,
    pub token_id: TokenID,
}

impl TokenPrice {
    pub fn new(
        sol_usd_price_account: &AccountInfo,
        token_usd_price_account: &AccountInfo,
        token_id: TokenID,
    ) -> Result<Self, ProgramError> {
        if token_id == 0 {
            Ok(Self::new_lamports())
        } else {
            let lamports = TOKENS[0];
            let token = TOKENS[token_id as usize];

            if lamports.pyth_usd_price_key != *sol_usd_price_account.key {
                return Err(TokenError::InvalidPriceAccount.into());
            }

            if token.pyth_usd_price_key != *token_usd_price_account.key {
                return Err(TokenError::InvalidPriceAccount.into());
            }

            let lamports_usd = Self::load_token_usd_price(sol_usd_price_account, 0)?;
            let token_usd = Self::load_token_usd_price(token_usd_price_account, token_id)?;

            Ok(Self::new_from_price(lamports_usd, token_usd, token_id))
        }
    }

    pub fn load_token_usd_price(
        token_usd_price_account: &AccountInfo,
        token_id: TokenID,
    ) -> Result<Price, TokenError> {
        let price_feed = load_price_feed_from_account_info(token_usd_price_account)
            .or(Err(TokenError::PriceError))?;

        let base_price = price_feed
            .get_current_price()
            .ok_or(TokenError::PriceError)?;

        let price = base_price
            .cmul(1, -(elusiv_token(token_id)?.price_base_exp as i32))
            .ok_or(TokenError::PriceError)?;

        Ok(price)
    }

    pub fn new_from_price(lamports_usd: Price, token_usd: Price, token_id: TokenID) -> Self {
        if token_id == 0 {
            Self::new_lamports()
        } else {
            Self {
                lamports_usd,
                token_usd,
                token_id,
            }
        }
    }

    pub fn new_from_sol_price(
        sol_usd: Price,
        token_usd: Price,
        token_id: TokenID,
    ) -> Result<Self, TokenError> {
        if token_id == 0 {
            Ok(Self::new_lamports())
        } else {
            let lamports_usd = sol_usd
                .cmul(1, -(elusiv_token(0)?.price_base_exp as i32))
                .ok_or(TokenError::PriceError)?;

            Ok(Self {
                lamports_usd,
                token_usd,
                token_id,
            })
        }
    }

    pub fn new_lamports() -> Self {
        Self {
            lamports_usd: Price {
                price: 1,
                conf: 0,
                expo: 0,
            },
            token_usd: Price {
                price: 1,
                conf: 0,
                expo: 0,
            },
            token_id: 0,
        }
    }

    pub fn token_into_lamports(&self, token: Token) -> Result<Lamports, TokenError> {
        if token.token_id() != self.token_id {
            return Err(TokenError::InvalidTokenID);
        }

        if self.token_id == 0 {
            return Ok(Lamports(token.amount()));
        }

        let usd = self
            .token_usd
            .mul(&Price {
                price: token.amount().try_into().unwrap(),
                conf: 0,
                expo: -(elusiv_token(self.token_id)?.decimals as i32),
            })
            .ok_or(TokenError::PriceError)?;
        let price = usd
            .get_price_in_quote(&self.lamports_usd, 0)
            .ok_or(TokenError::PriceError)?;
        Token::new_from_price(0, price, false)?.into_lamports()
    }

    pub fn lamports_into_token(
        &self,
        lamports: &Lamports,
        token_id: TokenID,
    ) -> Result<Token, TokenError> {
        if token_id != self.token_id {
            return Err(TokenError::InvalidTokenID);
        }

        if self.token_id == 0 {
            return Ok(lamports.into_token_strict());
        }

        let usd = self
            .lamports_usd
            .mul(&Price {
                price: lamports.0.try_into().unwrap(),
                conf: 0,
                expo: 0,
            })
            .ok_or(TokenError::PriceError)?;
        let price = usd
            .get_price_in_quote(
                &self.token_usd,
                -(elusiv_token(self.token_id)?.decimals as i32),
            )
            .ok_or(TokenError::PriceError)?;
        Token::new_from_price(token_id, price, false)
    }
}

#[cfg(feature = "test-elusiv")]
pub fn pyth_price_account_data(price: &Price) -> Result<Vec<u8>, TokenError> {
    use bytemuck::bytes_of;
    use pyth_sdk_solana::{
        state::{AccountType, MAGIC, VERSION_2},
        PriceStatus,
    };

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
pub fn spl_token_account_data(token_id: TokenID) -> Vec<u8> {
    let account = spl_token::state::Account {
        mint: elusiv_token(token_id).unwrap().mint,
        state: spl_token::state::AccountState::Initialized,
        ..Default::default()
    };
    let mut data = vec![0; spl_token::state::Account::LEN];
    spl_token::state::Account::pack(account, &mut data[..]).unwrap();
    data
}
