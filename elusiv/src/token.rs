pub use elusiv_types::tokens::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::macros::{account_info, pyth_price_account_info};
    use solana_program::{native_token::LAMPORTS_PER_SOL, pubkey::Pubkey};
    use std::{num::NonZeroU16, ops::Add, ops::Sub};

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
        assert_eq!(Token::new(0, 123), Token::Lamports(Lamports(123)));
        let id = NonZeroU16::new(99).unwrap();
        assert_eq!(
            Token::new(99, 456),
            Token::SPLToken(SPLToken { amount: 456, id })
        );
    }

    #[test]
    #[allow(unused_variables)]
    fn test_token_new_checked() {
        assert_eq!(
            Token::new_checked(TOKENS.len() as u16, 1_000_000),
            Err(TokenError::InvalidTokenID)
        );

        let min = lamports_token().min;
        let max = lamports_token().max;
        assert_eq!(
            Token::new_checked(0, max + 1),
            Err(TokenError::InvalidAmount)
        );
        assert_eq!(
            Token::new_checked(0, min - 1),
            Err(TokenError::InvalidAmount)
        );

        assert_eq!(
            Token::new_checked(0, lamports_token().max),
            Ok(Token::Lamports(Lamports(max)))
        );
        assert_eq!(
            Token::new_checked(0, lamports_token().min),
            Ok(Token::Lamports(Lamports(min)))
        );
    }

    #[test]
    fn test_token_new_from_price() {
        let price = Price {
            price: 123456,
            expo: -30,
            conf: 100,
        };
        assert_eq!(
            Token::new_from_price(0, price, true),
            Err(TokenError::InvalidAmount)
        );

        let price = Price {
            price: 123456789,
            expo: -2,
            conf: 100,
        };
        assert_eq!(
            Token::new_from_price(0, price, true),
            Ok(Token::Lamports(Lamports(1234567)))
        );
    }

    #[test]
    fn test_enforce_token_equality() {
        let a = Token::new(0, 1_000_000);
        let b = Token::new(1, 1_000_000);
        assert_eq!(
            a.enforce_token_equality(&b),
            Err(TokenError::MismatchedTokenID)
        );

        let a = Token::new(1, 1_000_000);
        assert_eq!(a.enforce_token_equality(&b), Ok(1));
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
        assert_eq!(Token::new(0, 10).into_lamports(), Ok(Lamports(10)));
        assert_eq!(
            Token::new(1, 10).into_lamports(),
            Err(TokenError::InvalidTokenID)
        );
    }

    #[test]
    fn test_add_tokens() {
        assert_eq!(
            Token::new(0, 10).add(Token::new(1, 10)),
            Err(TokenError::MismatchedTokenID)
        );
        assert_eq!(
            Token::new(0, u64::MAX).add(Token::new(0, 1)),
            Err(TokenError::Overflow)
        );
        assert_eq!(
            Token::new(0, 123).add(Token::new(0, 1_000)),
            Ok(Token::Lamports(Lamports(1_123)))
        );
    }

    #[test]
    fn test_sub_tokens() {
        assert_eq!(
            Token::new(0, 10).sub(Token::new(1, 10)),
            Err(TokenError::MismatchedTokenID)
        );
        assert_eq!(
            Token::new(0, 0).sub(Token::new(0, 1)),
            Err(TokenError::Underflow)
        );
        assert_eq!(
            Token::new(0, 123).sub(Token::new(0, 23)),
            Ok(Token::Lamports(Lamports(100)))
        );
    }

    #[test]
    fn test_lamports_into_token_strict() {
        assert_eq!(
            Lamports(123).into_token_strict(),
            Token::Lamports(Lamports(123))
        );
    }

    #[test]
    fn test_add_lamports() {
        assert_eq!(
            Lamports(u64::MAX).add(Lamports(1)),
            Err(TokenError::Overflow)
        );
        assert_eq!(Lamports(100).add(Lamports(23)), Ok(Lamports(123)));
    }

    #[test]
    #[allow(unused_variables)]
    fn test_spl_token_new() {
        assert_eq!(SPLToken::new(0, 10), Err(TokenError::InvalidTokenID));
        let id = NonZeroU16::new(1).unwrap();
        assert_eq!(SPLToken::new(1, 10), Ok(SPLToken { id, amount: 10 }));
    }

    #[test]
    fn test_verify_token_account() {
        account_info!(sol_account, Pubkey::new_unique(), vec![]);

        assert!(verify_token_account(&sol_account, 0).unwrap());
        assert!(!verify_token_account(&sol_account, 1).unwrap());

        let data = spl_token_account_data(USDC_TOKEN_ID);
        account_info!(usdc_account, Pubkey::new_unique(), data.clone());
        assert!(!verify_token_account(&usdc_account, 1).unwrap());

        account_info!(
            usdc_account,
            Pubkey::new_unique(),
            data,
            spl_token::id(),
            false
        );
        assert!(verify_token_account(&usdc_account, 1).unwrap());
        assert!(!verify_token_account(&usdc_account, 0).unwrap());
        assert!(!verify_token_account(&usdc_account, 2).unwrap());
    }

    #[test]
    fn test_token_price_new() {
        let sol_usd = Price {
            price: 39,
            conf: 1,
            expo: 0,
        }; // 1 SOL = 39 USD +- 1 USD
        pyth_price_account_info!(sol_usd_account, LAMPORTS_TOKEN_ID, sol_usd);

        let usdc_usd = Price {
            price: 1,
            conf: 1,
            expo: 0,
        }; // 1 USDC = 1 USD
        pyth_price_account_info!(usdc_usd_account, USDC_TOKEN_ID, usdc_usd);

        let price = TokenPrice::new(&sol_usd_account, &usdc_usd_account, USDC_TOKEN_ID).unwrap();

        assert_eq!(
            price.lamports_usd,
            Price {
                price: sol_usd.price,
                conf: sol_usd.conf,
                expo: -9
            }
        );
        assert_eq!(price.token_usd, usdc_usd);
    }

    #[test]
    fn test_load_token_usd_price() {
        let sol_usd = Price {
            price: 39,
            conf: 1,
            expo: 0,
        }; // 1 SOL = 39 USD +- 1 USD
        pyth_price_account_info!(sol_usd_account, LAMPORTS_TOKEN_ID, sol_usd);
        let lamports_usd =
            TokenPrice::load_token_usd_price(&sol_usd_account, LAMPORTS_TOKEN_ID).unwrap();
        assert_eq!(lamports_usd.price, sol_usd.price);
        assert_eq!(lamports_usd.conf, sol_usd.conf);
        assert_eq!(lamports_usd.expo, -9);

        let reduced = lamports_usd.scale_to_exponent(0).unwrap();
        assert_eq!(reduced.price, 0);
        assert_eq!(reduced.conf, 0);
    }

    #[test]
    fn test_token_price_new_from_price() {
        let lamports_usd = Price {
            price: 39,
            conf: 123,
            expo: -9,
        };
        let token_usd = Price {
            price: 1,
            conf: 45,
            expo: 0,
        };

        let price = TokenPrice::new_from_price(lamports_usd, token_usd, USDC_TOKEN_ID);

        assert_eq!(price.lamports_usd, lamports_usd);
        assert_eq!(price.token_usd, token_usd);
        assert_eq!(price.token_id, USDC_TOKEN_ID);
    }

    #[test]
    fn test_new_from_sol_price() {
        let sol_usd = Price {
            price: 390,
            conf: 123,
            expo: -1,
        };
        let token_usd = Price {
            price: 99,
            conf: 123,
            expo: -2,
        };

        let price = TokenPrice::new_from_sol_price(sol_usd, token_usd, USDT_TOKEN_ID).unwrap();

        let lamports_usd = Price {
            price: 390,
            conf: 123,
            expo: -10,
        };
        assert_eq!(price.lamports_usd, lamports_usd);
        assert_eq!(price.token_usd, token_usd);
        assert_eq!(price.token_id, USDT_TOKEN_ID);
    }

    #[test]
    fn test_token_price_new_lamports() {
        let price = TokenPrice::new_lamports();
        assert_eq!(
            price.token_into_lamports(Token::Lamports(Lamports(123))),
            Ok(Lamports(123))
        );
    }

    #[test]
    fn test_token_into_lamports() {
        // 1 LAMPORT = 39 * 10^{-9} USD
        let lamports_usd = Price {
            price: 39,
            conf: 0,
            expo: -9,
        };
        // 1 USDC = 0.5 USD
        let token_usd = Price {
            price: 500_000,
            conf: 0,
            expo: -6,
        };
        let price = TokenPrice::new_from_price(lamports_usd, token_usd, USDC_TOKEN_ID);

        // 1 USD = 1 / 39 * 10^{-9} LAMPORTS
        // 1 USDC = 0.5 * 1 / (39 * 10^{-9}) LAMPORTS (https://www.wolframalpha.com/input?i=0.5+*+1+%2F+%2839+*+power+%2810%2C+-9%29%29)
        assert_eq!(
            12820512,
            price
                .token_into_lamports(Token::new(USDC_TOKEN_ID, 1_000_000))
                .unwrap()
                .0
        );

        // 99 USDC = 0.5 * 99 * 1 / (39 * 10^{-9}) LAMPORTS (https://www.wolframalpha.com/input?i=0.5+*+99+*+1+%2F+%2839+*+power+%2810%2C+-9%29%29)
        assert_eq!(
            1269230769,
            price
                .token_into_lamports(Token::new(USDC_TOKEN_ID, 99_000_000))
                .unwrap()
                .0
        );
    }

    #[test]
    fn test_lamports_into_token() {
        // 1 LAMPORT = 39 * 10^{-9} USD
        let lamports_usd = Price {
            price: 39,
            conf: 0,
            expo: -9,
        };
        // 1 USDC = 0.5 USD
        let token_usd = Price {
            price: 500_000,
            conf: 0,
            expo: -6,
        };
        let price = TokenPrice::new_from_price(lamports_usd, token_usd, USDC_TOKEN_ID);

        // 1 LAMPORT = 39 * 10^{-9} * 2 USDC = 0.000_000_078 USDC (https://www.wolframalpha.com/input?i=2+*+39+*+power+%2810%2C+-9%29)
        assert_eq!(
            0,
            price
                .lamports_into_token(&Lamports(1), USDC_TOKEN_ID)
                .unwrap()
                .amount()
        );

        // 1_000 LAMPORTS = 1_000 * 39 * 10^{-9} * 2 USDC = 0.000_078 USDC (https://www.wolframalpha.com/input?i=1000+*+2+*+39+*+power+%2810%2C+-9%29)
        assert_eq!(
            78,
            price
                .lamports_into_token(&Lamports(1_000), USDC_TOKEN_ID)
                .unwrap()
                .amount()
        );

        // 99 SOL = 99 * 10^9 LAMPORTS = 99 * 10^9 * 39 * 10^{-9} * 2 USDC = 99 * 39 * 2 USDC
        assert_eq!(
            99 * 39 * 2 * 1_000_000,
            price
                .lamports_into_token(&Lamports(99 * LAMPORTS_PER_SOL), USDC_TOKEN_ID)
                .unwrap()
                .amount()
        );
    }

    #[test]
    fn test_pyth_price_account_data() {
        let price = Price {
            price: 123,
            conf: 456,
            expo: 7,
        };
        pyth_price_account_info!(sol_usd_account, LAMPORTS_TOKEN_ID, price);
        let price_feed = load_price_feed_from_account_info(&sol_usd_account).unwrap();
        assert_eq!(price, price_feed.get_current_price().unwrap());
        assert_eq!(
            *sol_usd_account.key,
            TOKENS[LAMPORTS_TOKEN_ID as usize].pyth_usd_price_key
        );
    }
}
