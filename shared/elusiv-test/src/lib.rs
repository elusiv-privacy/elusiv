#![allow(dead_code)]
#![allow(unused_macros)]

use elusiv_types::tokens::{
    elusiv_token, pyth_price_account_data, Lamports, Price, SPLToken, Token, TOKENS,
};
use elusiv_types::{
    EagerAccount, EagerAccountRepr, PDAAccount, PDAOffset, ParentAccount, SizedAccount,
    UserAccount, WritableUserAccount,
};
use solana_program::program_pack::Pack;
use solana_program::{
    instruction::{AccountMeta, Instruction, InstructionError},
    native_token::LAMPORTS_PER_SOL,
    program_option::COption,
    pubkey::Pubkey,
    system_instruction,
};
use solana_program_test::*;
use solana_sdk::{
    account::AccountSharedData, compute_budget::ComputeBudgetInstruction, signature::Keypair,
    signer::Signer, transaction::Transaction,
};
use spl_associated_token_account::instruction::create_associated_token_account;
use std::{collections::HashMap, process::Command, str::FromStr};

pub type ProcessInstructionWithContext =
    fn(usize, &[u8], &mut InvokeContext) -> Result<(), InstructionError>;
pub type Program = (String, Pubkey, Option<ProcessInstructionWithContext>);

pub struct ElusivProgramTest {
    context: ProgramTestContext,
    spl_tokens: Vec<u16>,
    programs: Vec<Program>,
}

impl ElusivProgramTest {
    pub async fn start(programs: &[Program]) -> Self {
        let mut test = ProgramTest::default();
        for (name, id, process_instruction) in programs.iter() {
            test.add_program(name, *id, *process_instruction);
        }
        let context = test.start_with_context().await;

        Self {
            context,
            spl_tokens: Vec::new(),
            programs: programs.to_vec(),
        }
    }

    pub async fn fork(&mut self, accounts: &[Pubkey]) -> Self {
        let mut n = Self::start(&self.programs).await;

        for account in accounts {
            if let Some(a) = self
                .context
                .banks_client
                .get_account(*account)
                .await
                .unwrap()
            {
                n.context.set_account(account, &a.into());
            }
        }

        for token_id in &self.spl_tokens {
            n.create_spl_token(*token_id).await;
        }

        n
    }

    pub async fn fork_for_instructions(&mut self, ixs: &[Instruction]) -> Self {
        let accounts = ixs
            .iter()
            .map(|ix| {
                ix.accounts
                    .iter()
                    .map(|a| a.pubkey)
                    .collect::<Vec<Pubkey>>()
            })
            .fold(Vec::new(), |acc, x| {
                let mut acc = acc;
                acc.extend(x);
                acc
            });

        self.fork(&accounts).await
    }

    pub async fn new_actor(&mut self) -> Actor {
        Actor::new(self).await
    }

    pub async fn process_transaction(
        &mut self,
        instructions: &[Instruction],
        signing_keypairs: &[&Keypair],
    ) -> Result<(), BanksClientError> {
        let mut signing_keypairs = signing_keypairs.to_vec();
        signing_keypairs.insert(0, &self.context.payer);

        let mut tx = Transaction::new_with_payer(instructions, Some(&self.context.payer.pubkey()));
        self.context.last_blockhash = self.context.banks_client.get_latest_blockhash().await?;

        tx.try_sign(&signing_keypairs, self.context.last_blockhash)
            .or(Err(BanksClientError::ClientError("Signature failure")))?;

        self.context
            .banks_client
            .process_transaction_with_preflight(tx)
            .await
    }

    pub async fn process_transaction_nonced(
        &mut self,
        instructions: &[Instruction],
        signing_keypairs: &[&Keypair],
    ) -> Result<(), BanksClientError> {
        let mut instructions = instructions.to_vec();
        instructions[0] = nonce_instruction(instructions[0].clone());
        self.process_transaction(&instructions, signing_keypairs)
            .await
    }

    pub fn context(&mut self) -> &mut ProgramTestContext {
        &mut self.context
    }

    pub fn payer(&self) -> Pubkey {
        self.context.payer.pubkey()
    }

    pub async fn account_does_exist(&mut self, address: &Pubkey) -> bool {
        matches!(
            self.context
                .banks_client
                .get_account(*address)
                .await
                .unwrap(),
            Some(_)
        )
    }

    pub async fn account_does_not_exist(&mut self, address: &Pubkey) -> bool {
        !self.account_does_exist(address).await
    }

    pub async fn lamports(&mut self, address: &Pubkey) -> Lamports {
        Lamports(
            self.context
                .banks_client
                .get_account(*address)
                .await
                .unwrap()
                .unwrap()
                .lamports,
        )
    }

    pub async fn pda_lamports(&mut self, address: &Pubkey, data_len: usize) -> Lamports {
        let lamports = self.lamports(address).await;
        let rent = self.rent(data_len).await;
        Lamports(lamports.0 - rent.0)
    }

    pub async fn spl_balance(&mut self, address: &Pubkey) -> u64 {
        let data = self.data(address).await;
        let state = spl_token::state::Account::unpack_unchecked(&data[..]).unwrap();
        state.amount
    }

    pub async fn data(&mut self, address: &Pubkey) -> Vec<u8> {
        self.context
            .banks_client
            .get_account(*address)
            .await
            .unwrap()
            .unwrap()
            .data
    }

    pub async fn rent(&mut self, data_len: usize) -> Lamports {
        let rent = self.context.banks_client.get_rent().await.unwrap();
        Lamports(rent.minimum_balance(data_len))
    }

    #[allow(deprecated)]
    pub async fn lamports_per_signature(&mut self) -> Lamports {
        Lamports(
            self.context
                .banks_client
                .get_fees()
                .await
                .unwrap()
                .0
                .lamports_per_signature,
        )
    }

    pub async fn create_spl_token(&mut self, token_id: u16) {
        assert!(token_id != 0);
        assert!(!self.spl_tokens.contains(&token_id));
        let token = TOKENS[token_id as usize];

        let supply = u64::MAX / 2;
        let mint = spl_token::state::Mint {
            mint_authority: COption::Some(self.context.payer.pubkey()),
            supply,
            decimals: token.decimals,
            is_initialized: true,
            freeze_authority: COption::Some(self.context.payer.pubkey()),
        };

        let mut data = vec![0; spl_token::state::Mint::LEN];
        mint.pack_into_slice(&mut data[..]);
        self.set_account_rent_exempt(&token.mint, &data[..], &spl_token::id())
            .await;
        self.spl_tokens.push(token_id);
    }

    pub async fn set_token_to_usd_price_pyth(&mut self, token_id: u16, price: Price) {
        let token = TOKENS[token_id as usize];
        let price_key = token.pyth_usd_price_key;
        let data = pyth_price_account_data(&price).unwrap();
        self.set_account_rent_exempt(&price_key, &data[..], &pyth_oracle_program())
            .await;
    }

    pub fn token_to_usd_price_pyth_account(&mut self, token_id: u16) -> Pubkey {
        TOKENS[token_id as usize].pyth_usd_price_key
    }

    pub async fn create_spl_token_account(&mut self, authority: &Pubkey, token_id: u16) -> Pubkey {
        assert!(token_id != 0);
        let token = TOKENS[token_id as usize];

        let token_account_keypair = Keypair::new();
        let rent = self.rent(spl_token::state::Account::LEN).await;
        let create_account_instruction = system_instruction::create_account(
            &self.context.payer.pubkey(),
            &token_account_keypair.pubkey(),
            rent.0,
            spl_token::state::Account::LEN as u64,
            &spl_token::id(),
        );

        let initialize_account_instruction = spl_token::instruction::initialize_account(
            &spl_token::id(),
            &token_account_keypair.pubkey(),
            &token.mint,
            authority,
        )
        .unwrap();

        self.process_transaction(
            &[create_account_instruction, initialize_account_instruction],
            &[&token_account_keypair],
        )
        .await
        .unwrap();

        token_account_keypair.pubkey()
    }

    pub async fn airdrop(&mut self, address: &Pubkey, token: Token) {
        match token {
            Token::Lamports(Lamports(lamports)) => {
                self.airdrop_lamports(address, lamports).await;
            }
            Token::SPLToken(SPLToken { amount, id }) => {
                self.mint_spl_token(address, amount, id.get()).await;
            }
        }
    }

    pub async fn airdrop_lamports(&mut self, address: &Pubkey, lamports: u64) {
        let instruction =
            system_instruction::transfer(&self.context.payer.pubkey(), address, lamports);
        self.process_transaction_nonced(&[instruction], &[])
            .await
            .unwrap();
    }

    pub async fn mint_spl_token(&mut self, address: &Pubkey, amount: u64, token_id: u16) {
        let token = TOKENS[token_id as usize];

        let mint_instruction = spl_token::instruction::mint_to(
            &spl_token::id(),
            &token.mint,
            address,
            &self.context.payer.pubkey(),
            &[],
            amount,
        )
        .unwrap();

        self.process_transaction(&[mint_instruction], &[])
            .await
            .unwrap();
    }

    pub async fn set_account(
        &mut self,
        address: &Pubkey,
        data: &[u8],
        lamports: Lamports,
        owner: &Pubkey,
    ) {
        let mut account_shared_data = AccountSharedData::new(lamports.0, data.len(), owner);

        account_shared_data.set_data(data.to_vec());
        self.context.set_account(address, &account_shared_data);
    }

    pub async fn set_program_account(
        &mut self,
        program_id: &Pubkey,
        address: &Pubkey,
        data: &[u8],
        lamports: Lamports,
    ) {
        self.set_account(address, data, lamports, program_id).await
    }

    pub async fn set_account_rent_exempt(&mut self, address: &Pubkey, data: &[u8], owner: &Pubkey) {
        let rent = self.rent(data.len()).await;
        self.set_account(address, data, rent, owner).await;
    }

    pub async fn set_program_account_rent_exempt(
        &mut self,
        program_id: &Pubkey,
        address: &Pubkey,
        data: &[u8],
    ) {
        self.set_account_rent_exempt(address, data, program_id)
            .await
    }

    pub async fn create_account(
        &mut self,
        space: u64,
        lamports: Lamports,
        owner: &Pubkey,
    ) -> Keypair {
        let new_account_keypair = Keypair::new();
        let ix = solana_program::system_instruction::create_account(
            &self.context.payer.pubkey(),
            &new_account_keypair.pubkey(),
            lamports.0,
            space,
            owner,
        );

        assert!(self
            .process_transaction(&[ix], &[&new_account_keypair])
            .await
            .is_ok());

        new_account_keypair
    }

    pub async fn create_program_account_rent_exempt(
        &mut self,
        program_id: &Pubkey,
        data_len: usize,
    ) -> Keypair {
        let rent = self.rent(data_len).await;
        self.create_account(data_len as u64, rent, program_id).await
    }

    pub async fn create_program_account_empty(&mut self, program_id: &Pubkey) -> Keypair {
        self.create_account(0, Lamports(0), program_id).await
    }

    pub async fn create_parent_account<'a, 'b, 't, T: ParentAccount<'a, 'b, 't>>(
        &mut self,
        program_id: &Pubkey,
    ) -> Vec<Pubkey> {
        let mut result = Vec::new();

        for _ in 0..T::COUNT {
            let pk = self
                .create_program_account_rent_exempt(program_id, T::Child::SIZE)
                .await
                .pubkey();
            result.push(pk);
        }

        result
    }

    pub async fn tx_should_succeed(&mut self, ixs: &[Instruction], signers: &[&Keypair]) {
        assert!(self.process_transaction_nonced(ixs, signers).await.is_ok());
    }

    pub async fn tx_should_succeed_simple(&mut self, ixs: &[Instruction]) {
        assert!(self.process_transaction_nonced(ixs, &[]).await.is_ok());
    }

    pub async fn ix_should_succeed(&mut self, ix: Instruction, signers: &[&Keypair]) {
        self.tx_should_succeed(&[ix], signers).await
    }

    pub async fn ix_should_succeed_simple(&mut self, ix: Instruction) {
        assert!(self.process_transaction_nonced(&[ix], &[]).await.is_ok());
    }

    pub async fn tx_should_fail(
        &mut self,
        ixs: &[Instruction],
        signers: &[&Keypair],
    ) -> BanksClientError {
        self.process_transaction_nonced(ixs, signers)
            .await
            .unwrap_err()
    }

    pub async fn tx_should_fail_simple(&mut self, ixs: &[Instruction]) {
        assert!(self.process_transaction_nonced(ixs, &[]).await.is_err());
    }

    pub async fn ix_should_fail(
        &mut self,
        ix: Instruction,
        signers: &[&Keypair],
    ) -> BanksClientError {
        self.tx_should_fail(&[ix], signers).await
    }

    pub async fn ix_should_fail_simple(&mut self, ix: Instruction) {
        assert!(self.process_transaction_nonced(&[ix], &[]).await.is_err());
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
            let signer = if !acc.is_signer {
                (*original_signer).clone()
            } else {
                Actor::new(self).await
            };
            let mut ix = ix.clone();

            // Clone data and lamports
            let address = acc.pubkey;
            let accounts_exists = self.account_does_exist(&address).await;
            let data = if accounts_exists {
                self.data(&address).await
            } else {
                vec![]
            };
            let lamports = if accounts_exists {
                self.lamports(&address).await
            } else {
                Lamports(100_000)
            };
            let new_pubkey = Pubkey::new_unique();

            // TODO: owner fuzzing

            let owner = self
                .context
                .banks_client
                .get_account(address)
                .await
                .unwrap()
                .unwrap()
                .owner;
            self.set_account(&new_pubkey, &data[..], lamports, &owner)
                .await;

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
        let invalid_instructions = self.invalid_accounts_fuzzing(&valid_ix, signer).await;

        for (ix, signer) in invalid_instructions {
            let mut ixs = prefix_ixs.to_vec();
            ixs.push(ix);

            let signer = signer.clone();
            self.tx_should_fail(&ixs, &[&signer.keypair]).await;
        }

        let mut ixs = prefix_ixs.to_vec();
        ixs.push(valid_ix);
        self.tx_should_succeed(&ixs, &[&signer.keypair]).await;
    }

    pub async fn set_pda_account<A: SizedAccount + PDAAccount, F>(
        &mut self,
        program_id: &Pubkey,
        pda_pubkey: Option<Pubkey>,
        pda_offset: PDAOffset,
        setup: F,
    ) where
        F: FnOnce(&mut [u8]),
    {
        let data_len = A::SIZE;
        let address = A::find_with_pubkey_optional(pda_pubkey, pda_offset).0;
        let mut data = self.data(&address).await;

        setup(&mut data);

        let rent_exemption = self.rent(data_len).await;
        self.set_program_account(program_id, &address, &data, rent_exemption)
            .await;
    }

    pub async fn child_accounts<'a, P: ParentAccount<'a, 'a, 'a> + PDAAccount>(
        &mut self,
        data: &'a mut [u8],
    ) -> Vec<Pubkey> {
        let parent = P::new(data).unwrap();
        (0..P::COUNT)
            .map(|i| parent.get_child_pubkey(i).unwrap())
            .collect()
    }

    pub async fn eager_account<
        'a,
        A: EagerAccount<'a, Repr = B> + PDAAccount,
        B: EagerAccountRepr,
    >(
        &mut self,
        offset: PDAOffset,
    ) -> B {
        let data = self.data(&A::find(offset).0).await;
        B::new(data).unwrap()
    }

    pub async fn eager_account2<
        'a,
        A: EagerAccount<'a, Repr = B> + PDAAccount,
        B: EagerAccountRepr,
    >(
        &mut self,
        pubkey: Pubkey,
        offset: PDAOffset,
    ) -> B {
        let data = self.data(&A::find_with_pubkey(pubkey, offset).0).await;
        B::new(data).unwrap()
    }
}

pub fn user_accounts(pubkeys: &[Pubkey]) -> Vec<UserAccount> {
    pubkeys.iter().map(|p| UserAccount(*p)).collect()
}

pub fn writable_user_accounts(pubkeys: &[Pubkey]) -> Vec<WritableUserAccount> {
    pubkeys.iter().map(|p| WritableUserAccount(*p)).collect()
}

const DEFAULT_START_BALANCE: u64 = LAMPORTS_PER_SOL;

pub struct Actor {
    pub keypair: Keypair,
    pub pubkey: Pubkey,
    token_accounts: HashMap<u16, Pubkey>,

    // Due to the InvalidRentPayingAccount error, we need to give our client a starting balance (= zero)
    pub start_balance: u64,
}

impl Clone for Actor {
    fn clone(&self) -> Self {
        Actor {
            keypair: Keypair::from_bytes(&self.keypair.to_bytes()).unwrap(),
            pubkey: self.pubkey,
            token_accounts: self.token_accounts.clone(),
            start_balance: self.start_balance,
        }
    }
}

impl Actor {
    pub async fn new(test: &mut ElusivProgramTest) -> Self {
        let keypair = Keypair::new();
        let pubkey = keypair.pubkey();

        test.airdrop_lamports(&pubkey, DEFAULT_START_BALANCE).await;

        Actor {
            keypair,
            pubkey,
            token_accounts: HashMap::new(),
            start_balance: DEFAULT_START_BALANCE,
        }
    }

    pub async fn open_token_account(
        &mut self,
        token_id: u16,
        amount: u64,
        test: &mut ElusivProgramTest,
    ) {
        let account = test.create_spl_token_account(&self.pubkey, token_id).await;
        if amount > 0 {
            test.airdrop(&account, Token::new_checked(token_id, amount).unwrap())
                .await;
        }
        self.token_accounts.insert(token_id, account);
    }

    pub fn get_token_account(&self, token_id: u16) -> Pubkey {
        self.token_accounts[&token_id]
    }

    pub async fn airdrop(&self, token_id: u16, amount: u64, test: &mut ElusivProgramTest) {
        if token_id == 0 {
            test.airdrop_lamports(&self.pubkey, amount).await;
        } else {
            let account = self.token_accounts.get(&token_id).unwrap();
            test.airdrop(account, Token::new(token_id, amount)).await;
        }
    }

    /// Returns the account's balance - start_balance - failed_signatures * lamports_per_signature
    pub async fn balance(&self, token_id: u16, test: &mut ElusivProgramTest) -> u64 {
        if token_id == 0 {
            self.lamports(test).await
        } else {
            let address = self.token_accounts.get(&token_id).unwrap();
            test.spl_balance(address).await
        }
    }

    pub async fn lamports(&self, test: &mut ElusivProgramTest) -> u64 {
        test.lamports(&self.pubkey).await.0 - self.start_balance
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
    ComputeBudgetInstruction::set_compute_unit_limit(count)
}

pub fn request_max_compute_units() -> Instruction {
    request_compute_units(1_400_000)
}

fn pyth_oracle_program() -> Pubkey {
    Pubkey::from_str("FsJ3A3u2vn5cTVofAjvy6y5kwABJAqYWpe4975bi2epH").unwrap()
}

pub async fn enable_program_token_account<A: PDAAccount>(
    test: &mut ElusivProgramTest,
    token_id: u16,
    offset: PDAOffset,
) {
    let ix = create_associated_token_account(
        &test.payer(),
        &A::find(offset).0,
        &elusiv_token(token_id).unwrap().mint,
        &spl_token::id(),
    );
    test.process_transaction(&[ix], &[]).await.unwrap();
}

pub fn compile_mock_program() {
    if std::path::Path::new("../lib/mock_program.so").exists() {
        return;
    }

    Command::new("cargo")
        .args([
            "build-bpf",
            "--manifest-path=./shared/elusiv-test/mock-program/Cargo.toml",
            "--bpf-out-dir=../lib",
        ])
        .spawn()
        .unwrap()
        .wait()
        .unwrap();
}
