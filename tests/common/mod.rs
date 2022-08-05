#![allow(dead_code)]
#![allow(unused_macros)]

pub mod log;

use pyth_sdk_solana::Price;
use solana_program::{
    pubkey::Pubkey,
    instruction::{Instruction, AccountMeta}, system_instruction, native_token::LAMPORTS_PER_SOL, program_option::COption,
};
use solana_program_test::*;
use solana_program::program_pack::Pack;
use solana_sdk::{signature::{Keypair}, transaction::Transaction, signer::Signer, account::AccountSharedData, compute_budget::ComputeBudgetInstruction};
use assert_matches::assert_matches;
use elusiv::{token::{TOKENS, pyth_price_account_data}, process_instruction};

pub struct ElusivProgramTest {
    context: ProgramTestContext,
}

impl ElusivProgramTest {
    pub async fn start() -> Self {
        let mut test = ProgramTest::default();
        let program_id = elusiv::id();
        test.add_program("elusiv", program_id, processor!(process_instruction));
        let context = test.start_with_context().await;

        Self { context }
    }

    pub async fn account_does_exist(&mut self, address: &Pubkey) -> bool {
        matches!(self.context.banks_client.get_account(*address).await.unwrap(), Some(_))
    }

    pub async fn account_does_not_exist(&mut self, address: &Pubkey) -> bool {
        !self.account_does_exist(address).await
    }

    pub async fn balance(&mut self, address: &Pubkey) -> u64 {
        self.context.banks_client.get_account(*address).await.unwrap().unwrap().lamports
    }

    pub async fn data(&mut self, address: &Pubkey) -> Vec<u8> {
        self.context.banks_client.get_account(*address).await.unwrap().unwrap().data
    }

    pub async fn rent(&mut self, data_len: usize) -> u64 {
        let rent = self.context.banks_client.get_rent().await.unwrap();
        rent.minimum_balance(data_len)
    }

    #[allow(deprecated)]
    pub async fn lamports_per_signature(&mut self) -> u64 {
        self.context.banks_client.get_fees().await.unwrap().0.lamports_per_signature
    }

    pub async fn airdrop(&mut self, address: &Pubkey, lamports: u64) {
        let mut tx = Transaction::new_with_payer(
            &[
                nonce_instruction(
                    system_instruction::transfer(
                        &self.context.payer.pubkey(),
                        address,
                        lamports,
                    )
                )
            ],
            Some(&self.context.payer.pubkey())
        );
        tx.sign(&[&self.context.payer], self.context.last_blockhash);
        assert_matches!(self.context.banks_client.process_transaction(tx).await, Ok(()));
    }

    pub async fn create_spl_token(&mut self, token_id: u16) {
        assert!(token_id != 0);
        let token = TOKENS[token_id as usize];

        let mint = spl_token::state::Mint {
            mint_authority: COption::None,
            supply: u64::MAX / 2,
            decimals: token.decimals,
            is_initialized: true,
            freeze_authority: COption::None,
        };

        let mut data = vec![0; spl_token::state::Mint::LEN];
        mint.pack_into_slice(&mut data[..]);

        self.set_account_rent_exempt(&token.mint, &data[..]).await;
    }

    pub async fn set_token_to_usd_price_pyth(
        &mut self,
        token_id: u16,
        price: Price,
    ) {
        let token = TOKENS[token_id as usize]; 
        let price_key = token.pyth_usd_price_key;
        let data = pyth_price_account_data(price, token_id).unwrap();
        self.set_account_rent_exempt(&price_key, &data[..]).await;
    }

    async fn create_spl_token_account(
        &mut self,
        authority: &Pubkey,
        token_id: u16,
    ) {
        assert!(token_id != 0);
        let token = TOKENS[token_id as usize];
    
        let token_account_keypair = Keypair::new();
        let rent = self.rent(spl_token::state::Account::LEN).await;
        let create_account_instruction = system_instruction::create_account(
            &self.context.payer.pubkey(),
            &token_account_keypair.pubkey(),
            rent,
            spl_token::state::Account::get_packed_len() as u64,
            &spl_token::id(),
        );
    
        let initialize_account_instruction = spl_token::instruction::initialize_account(
            &spl_token::id(),
            &token_account_keypair.pubkey(),
            &token.mint,
            authority,
        ).unwrap();
    
        self.context.banks_client.process_transaction(
            Transaction::new_signed_with_payer(
                &[
                    create_account_instruction,
                    initialize_account_instruction,
                ],
                Some(&self.context.payer.pubkey()),
                &[&token_account_keypair],
                self.context.last_blockhash,
            )
        ).await.unwrap();
    }

    pub async fn set_account(
        &mut self,
        address: &Pubkey,
        data: &[u8],
        lamports: u64,
    ) {
        let mut account_shared_data = AccountSharedData::new(
            lamports,
            data.len(),
            &elusiv::id()
        );

        account_shared_data.set_data(data.to_vec());
        self.context.set_account(address, &account_shared_data);
    }

    pub async fn set_account_rent_exempt(
        &mut self,
        address: &Pubkey,
        data: &[u8],
    ) {
        let rent = self.rent(data.len()).await;
        self.set_account(address, data, rent).await;
    }
    
    pub async fn create_account(
        &mut self,
        space: u64,
        lamports: u64,
    ) -> Keypair {
        let new_account_keypair = Keypair::new();
        let ix = solana_program::system_instruction::create_account(
            &self.context.payer.pubkey(),
            &new_account_keypair.pubkey(),
            space,
            lamports,
            &new_account_keypair.pubkey(),
        );
    
        let transaction = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.context.payer.pubkey()),
            &[&self.context.payer, &new_account_keypair],
            self.context.last_blockhash,
        );
        assert_matches!(self.context.banks_client.process_transaction(transaction).await, Ok(()));
    
        new_account_keypair
    }

    pub async fn create_account_rent_exempt(&mut self, data_len: usize) -> Keypair {
        let rent = self.rent(data_len).await;
        self.create_account(data_len as u64, rent).await
    }

    pub async fn create_account_empty(&mut self) -> Keypair {
        self.create_account(0, 0).await
    }

    async fn generate_and_sign_tx(
        &mut self,
        ixs: &[Instruction],
        signer: &mut Actor,
    ) -> Transaction {
        let ixs: Vec<Instruction> = ixs.iter()
            .map(|ix| nonce_instruction(ix.clone()))
            .collect();
        let mut tx = Transaction::new_with_payer(
            &ixs,
            Some(&signer.pubkey)
        );
        tx.sign(
            &[&signer.keypair],
            self.context.banks_client.get_latest_blockhash().await.unwrap()
        );

        tx
    }

    pub async fn tx_should_succeed(
        &mut self,
        ixs: &[Instruction],
        signer: &mut Actor,
    ) {
        let tx = self.generate_and_sign_tx(ixs, signer).await;
        assert_matches!(self.context.banks_client.process_transaction(tx).await, Ok(()));
    }
    
    pub async fn ix_should_succeed(
        &mut self,
        ix: Instruction,
        signer: &mut Actor,
    ) {
        self.tx_should_succeed(&[ix], signer).await
    }
    
    pub async fn tx_should_fail(
        &mut self,
        ixs: &[Instruction],
        signer: &mut Actor,
    ) {
        let tx = self.generate_and_sign_tx(ixs, signer).await;
        assert_matches!(self.context.banks_client.process_transaction(tx).await, Err(_));
    
        // To compensate for failure, we airdrop
        let lamports = self.lamports_per_signature().await;
        self.airdrop(&signer.pubkey, lamports).await;
    }
    
    pub async fn ix_should_fail(
        &mut self,
        ix: Instruction,
        signer: &mut Actor,
    ) {
        self.tx_should_fail(&[ix], signer).await
    }

    /// Replaces all accounts through invalid accounts with valid data and lamports
    /// - returns the fuzzed instructions and accorsing signers
    pub async fn invalid_accounts_fuzzing(
        &mut self,
        ix: &Instruction,
        original_signer: &Actor,
    ) -> Vec<(Instruction, Actor)> {
        let mut result = Vec::new();
        for (i, acc) in ix.accounts.iter().enumerate() {
            let signer = if !acc.is_signer { (*original_signer).clone() } else { Actor::new(self).await };
            let mut ix = ix.clone();

            // Clone data and lamports
            let address = acc.pubkey;
            let accounts_exists = self.account_does_exist(&address).await;
            let data = if accounts_exists { self.data(&address).await } else { vec![] };
            let lamports = if accounts_exists { self.balance(&address).await } else { 100_000 };
            let new_pubkey = Pubkey::new_unique();
            self.set_account(&new_pubkey, &data[..], lamports).await;

            if acc.is_writable {
                ix.accounts[i] = AccountMeta::new(new_pubkey, false);
            } else {
                ix.accounts[i] = AccountMeta::new_readonly(new_pubkey, false);
            }

            result.push((ix, signer));
        }
        result
    }

    /// All fuzzed ix variants should fail and the original ix should afterwards succeed
    /// - prefix_ixs are not fuzzed
    pub async fn test_instruction_fuzzing(
        &mut self,
        prefix_ixs: &[Instruction],
        valid_ix: Instruction,
        signer: &mut Actor,
    ) {
        let invalid_instructions = self.invalid_accounts_fuzzing(
            &valid_ix,
            signer,
        ).await;

        for (ix, signer) in invalid_instructions {
            let mut ixs = prefix_ixs.to_vec();
            ixs.push(ix);

            let mut signer = signer.clone();
            self.tx_should_fail(&ixs, &mut signer).await;
        }

        let mut ixs = prefix_ixs.to_vec();
        ixs.push(valid_ix);
        self.tx_should_succeed(&ixs, signer).await;
    }
}

const DEFAULT_START_BALANCE: u64 = LAMPORTS_PER_SOL;

pub struct Actor {
    pub keypair: Keypair,
    pub pubkey: Pubkey,

    // Due to the InvalidRentPayingAccount error, we need to give our client a starting balance (= zero)
    pub start_balance: u64,
}

impl Clone for Actor {
    fn clone(&self) -> Self {
        let keypair = Keypair::from_bytes(&self.keypair.to_bytes()).unwrap();
        Actor { keypair, pubkey: self.pubkey, start_balance: self.start_balance }
    }
}

impl Actor {
    pub async fn new(test: &mut ElusivProgramTest) -> Self {
        let keypair = Keypair::new();
        let pubkey = keypair.pubkey();

        test.airdrop(&pubkey, DEFAULT_START_BALANCE).await;

        Actor {
            keypair,
            pubkey,
            start_balance: DEFAULT_START_BALANCE,
        }
    }

    /// Returns the account's balance - start_balance - failed_signatures * lamports_per_signature
    pub async fn balance(&self, test: &mut ElusivProgramTest) -> u64 {
        test.balance(&self.pubkey).await - self.start_balance
    }

    pub async fn airdrop(&self, lamports: u64, test: &mut ElusivProgramTest) {
        test.airdrop(&self.pubkey, lamports).await
    }
}

/// Adds random nonce bytes at the end of the ix data
/// - prevents rejection of previously failed ix times without repeated execution
pub fn nonce_instruction(ix: Instruction) -> Instruction {
    let mut ix = ix;
    for _ in 0..8 {
        ix.data.push(rand::random());
    }
    ix
}

// Fee for CUs: https://github.com/solana-labs/solana/blob/3d9874b95a4bda9bb99cb067f168811296d208cc/sdk/src/fee.rs
pub fn request_compute_units(count: u32) -> Instruction {
    ComputeBudgetInstruction::request_units(count, 0)
}