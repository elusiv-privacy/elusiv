#![allow(dead_code)]
#![allow(unused_macros)]

use elusiv_computation::PartialComputation;
use elusiv_types::{PDAOffset, ParentAccount};
use spl_associated_token_account::instruction::create_associated_token_account;
use std::{collections::HashMap, str::FromStr};
use pyth_sdk_solana::Price;
use solana_program::{
    pubkey::Pubkey,
    instruction::{Instruction, AccountMeta}, system_instruction, native_token::LAMPORTS_PER_SOL, program_option::COption,
};
use solana_program_test::*;
use solana_program::program_pack::Pack;
use solana_sdk::{signature::Keypair, transaction::Transaction, signer::Signer, account::AccountSharedData, compute_budget::ComputeBudgetInstruction};
use assert_matches::assert_matches;
use elusiv::{token::{TOKENS, pyth_price_account_data, Token, Lamports, SPLToken, elusiv_token}, process_instruction, instruction::{open_all_initial_accounts, ElusivInstruction, WritableSignerAccount, WritableUserAccount, UserAccount}, state::{fee::{ProgramFee, BasisPointFee}, program_account::{SizedAccount, PDAAccount}, StorageAccount, NullifierAccount, governor::{PoolAccount, FeeCollectorAccount}}, proof::{CombinedMillerLoop, FinalExponentiation}, processor::{SingleInstancePDAAccountKind, MultiInstancePDAAccountKind}, fields::fr_to_u256_le, types::U256};

pub struct ElusivProgramTest {
    context: ProgramTestContext,
    spl_tokens: Vec<u16>,
}

impl ElusivProgramTest {
    pub async fn start() -> Self {
        let mut test = ProgramTest::default();
        let program_id = elusiv::id();
        test.add_program("elusiv", program_id, processor!(process_instruction));
        let context = test.start_with_context().await;

        Self {
            context,
            spl_tokens: Vec::new(),
        }
    }

    pub async fn fork(&mut self, accounts: &[Pubkey]) -> Self {
        let mut n = Self::start().await;

        for account in accounts {
            if let Some(a) = self.context.banks_client.get_account(*account).await.unwrap() {
                n.context.set_account(account, &a.into());
            }
        }

        for token_id in &self.spl_tokens {
            n.create_spl_token(*token_id, false).await;
        }

        n
    }

    pub async fn fork_for_instructions(&mut self, ixs: &[Instruction]) -> Self {
        let accounts = ixs.iter()
            .map(|ix| {
                ix.accounts.iter()
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

    pub async fn start_with_setup() -> Self {
        let mut test = Self::start().await;
        let genesis_fee = test.genesis_fee().await;

        test.setup_initial_pdas().await;
        test.setup_fee(0, genesis_fee).await;

        test
    }

    pub async fn setup_initial_pdas(&mut self) {
        let ixs = open_all_initial_accounts(self.context().payer.pubkey());
        self.tx_should_succeed_simple(&ixs).await;
    }

    pub async fn setup_fee(&mut self, fee_version: u32, program_fee: ProgramFee) {
        let ix = ElusivInstruction::init_new_fee_version_instruction(
            fee_version,
            program_fee,
            WritableSignerAccount(self.context.payer.pubkey()),
        );
        self.ix_should_succeed_simple(ix).await;
    }

    pub async fn setup_storage_account(&mut self) -> Vec<Pubkey> {
        self.ix_should_succeed_simple(
            ElusivInstruction::open_single_instance_account_instruction(
                SingleInstancePDAAccountKind::StorageAccount,
                WritableSignerAccount(self.context.payer.pubkey()),
                WritableUserAccount(StorageAccount::find(None).0),
            )
        ).await;
    
        let mut instructions = Vec::new();
        let pubkeys = self.create_parent_account::<StorageAccount>().await;
        for (i, p) in pubkeys.iter().enumerate() {
            instructions.push(
                ElusivInstruction::enable_storage_child_account_instruction(
                    i as u32,
                    WritableUserAccount(*p),
                )
            );
        }
        self.tx_should_succeed_simple(&instructions).await;
    
        pubkeys
    }

    pub async fn create_merkle_tree(&mut self, mt_index: u32) -> Vec<Pubkey> {
        let mut instructions = vec![
            ElusivInstruction::open_multi_instance_account_instruction(
                MultiInstancePDAAccountKind::NullifierAccount,
                mt_index,
                WritableSignerAccount(self.payer()),
                WritableUserAccount(NullifierAccount::find(Some(mt_index)).0)
            )
        ];
    
        let pubkeys = self.create_parent_account::<NullifierAccount>().await;
        for (i, p) in pubkeys.iter().enumerate() {
            instructions.push(
                ElusivInstruction::enable_nullifier_child_account_instruction(
                    mt_index,
                    i as u32,
                    WritableUserAccount(*p),
                )
            );
        }
        self.tx_should_succeed_simple(&instructions).await;
    
        pubkeys
    }

    pub async fn process_transaction(
        &mut self,
        instructions: &[Instruction],
        signing_keypairs: &[&Keypair],
    ) -> Result<(), BanksClientError> {
        let mut signing_keypairs = signing_keypairs.to_vec();
        signing_keypairs.insert(0, &self.context.payer);

        let mut tx = Transaction::new_with_payer(instructions, Some(&self.context.payer.pubkey()));
        tx.try_sign(&signing_keypairs, self.context.last_blockhash)
            .or(Err(BanksClientError::ClientError("Signature failure")))?;

        self.context.banks_client.process_transaction(tx).await
    }

    pub async fn process_transaction_nonced(
        &mut self,
        instructions: &[Instruction],
        signing_keypairs: &[&Keypair],
    ) -> Result<(), BanksClientError> {
        let instructions: Vec<Instruction> = instructions.iter()
            .map(|ix| nonce_instruction(ix.clone()))
            .collect();

        self.process_transaction(&instructions, signing_keypairs).await
    }

    pub fn context(&mut self) -> &mut ProgramTestContext {
        &mut self.context
    }

    pub fn payer(&self) -> Pubkey {
        self.context.payer.pubkey()
    }

    pub async fn account_does_exist(&mut self, address: &Pubkey) -> bool {
        matches!(self.context.banks_client.get_account(*address).await.unwrap(), Some(_))
    }

    pub async fn account_does_not_exist(&mut self, address: &Pubkey) -> bool {
        !self.account_does_exist(address).await
    }

    pub async fn lamports(&mut self, address: &Pubkey) -> Lamports {
        Lamports(
            self.context.banks_client.get_account(*address).await.unwrap().unwrap().lamports
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
        self.context.banks_client.get_account(*address).await.unwrap().unwrap().data
    }

    pub async fn rent(&mut self, data_len: usize) -> Lamports {
        let rent = self.context.banks_client.get_rent().await.unwrap();
        Lamports(rent.minimum_balance(data_len))
    }

    #[allow(deprecated)]
    pub async fn lamports_per_signature(&mut self) -> Lamports {
        Lamports(
            self.context.banks_client.get_fees().await.unwrap().0.lamports_per_signature
        )
    }

    pub async fn create_spl_token(&mut self, token_id: u16, enable_program_token_accounts: bool) {
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
        self.set_account_rent_exempt(&token.mint, &data[..], &spl_token::id()).await;
        self.spl_tokens.push(token_id);

        if enable_program_token_accounts {
            enable_program_token_account::<PoolAccount>(self, token_id, None).await;
            enable_program_token_account::<FeeCollectorAccount>(self, token_id, None).await;
        }
    }

    pub async fn set_token_to_usd_price_pyth(
        &mut self,
        token_id: u16,
        price: Price,
    ) {
        let token = TOKENS[token_id as usize]; 
        let price_key = token.pyth_usd_price_key;
        let data = pyth_price_account_data(&price).unwrap();
        self.set_account_rent_exempt(&price_key, &data[..], &pyth_oracle_program()).await;
    }

    pub fn token_to_usd_price_pyth_account(
        &mut self,
        token_id: u16,
    ) -> Pubkey {
        TOKENS[token_id as usize].pyth_usd_price_key
    }

    pub async fn create_spl_token_account(
        &mut self,
        authority: &Pubkey,
        token_id: u16,
    ) -> Pubkey {
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
        ).unwrap();
    
        self.process_transaction(
            &[
                create_account_instruction,
                initialize_account_instruction,
            ],
            &[&token_account_keypair],
        ).await.unwrap();

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
        let instruction = system_instruction::transfer(
            &self.context.payer.pubkey(),
            address,
            lamports,
        );
        self.process_transaction_nonced(&[instruction], &[]).await.unwrap();
    }

    pub async fn mint_spl_token(
        &mut self,
        address: &Pubkey,
        amount: u64,
        token_id: u16,
    ) {
        if !self.spl_tokens.contains(&token_id) {
            self.create_spl_token(token_id, true).await;
        }

        let token = TOKENS[token_id as usize];

        let mint_instruction = spl_token::instruction::mint_to(
            &spl_token::id(),
            &token.mint,
            address,
            &self.context.payer.pubkey(),
            &[],
            amount,
        ).unwrap();

        self.process_transaction(&[mint_instruction], &[]).await.unwrap();
    }

    pub async fn set_account(
        &mut self,
        address: &Pubkey,
        data: &[u8],
        lamports: Lamports,
        owner: &Pubkey,
    ) {
        let mut account_shared_data = AccountSharedData::new(
            lamports.0,
            data.len(),
            owner,
        );

        account_shared_data.set_data(data.to_vec());
        self.context.set_account(address, &account_shared_data);
    }

    pub async fn set_program_account(
        &mut self,
        address: &Pubkey,
        data: &[u8],
        lamports: Lamports,
    ) {
        self.set_account(address, data, lamports, &elusiv::id()).await
    }

    pub async fn set_account_rent_exempt(
        &mut self,
        address: &Pubkey,
        data: &[u8],
        owner: &Pubkey,
    ) {
        let rent = self.rent(data.len()).await;
        self.set_account(address, data, rent, owner).await;
    }

    pub async fn set_program_account_rent_exempt(
        &mut self,
        address: &Pubkey,
        data: &[u8],
    ) {
        self.set_account_rent_exempt(address, data, &elusiv::id()).await
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
    
        assert_matches!(
            self.process_transaction(&[ix], &[&new_account_keypair]).await,
            Ok(())
        );
    
        new_account_keypair
    }

    pub async fn create_program_account_rent_exempt(&mut self, data_len: usize) -> Keypair {
        let rent = self.rent(data_len).await;
        self.create_account(data_len as u64, rent, &elusiv::id()).await
    }

    pub async fn create_program_account_empty(&mut self) -> Keypair {
        self.create_account(0, Lamports(0), &elusiv::id()).await
    }

    pub async fn create_parent_account<'a, 'b, 't, T: ParentAccount<'a, 'b, 't>>(&mut self) -> Vec<Pubkey> {
        let mut result = Vec::new();
    
        for _ in 0..T::COUNT {
            let pk = self.create_program_account_rent_exempt(T::Child::SIZE).await.pubkey();
            result.push(pk);
        }
    
        result
    }

    pub async fn tx_should_succeed(
        &mut self,
        ixs: &[Instruction],
        signers: &[&Keypair],
    ) {
        assert_matches!(
            self.process_transaction_nonced(ixs, signers).await,
            Ok(())
        );
    }

    pub async fn tx_should_succeed_simple(&mut self, ixs: &[Instruction]) {
        assert_matches!(self.process_transaction_nonced(ixs, &[]).await, Ok(()));
    }
    
    pub async fn ix_should_succeed(
        &mut self,
        ix: Instruction,
        signers: &[&Keypair],
    ) {
        self.tx_should_succeed(&[ix], signers).await
    }

    pub async fn ix_should_succeed_simple(&mut self, ix: Instruction) {
        assert_matches!(self.process_transaction_nonced(&[ix], &[]).await, Ok(()));
    }
    
    pub async fn tx_should_fail(
        &mut self,
        ixs: &[Instruction],
        signers: &[&Keypair],
    ) {
        assert_matches!(
            self.process_transaction_nonced(ixs, signers).await,
            Err(_)
        );
    }

    pub async fn tx_should_fail_simple(&mut self, ixs: &[Instruction]) {
        assert_matches!(self.process_transaction_nonced(ixs, &[]).await, Err(_));
    }
    
    pub async fn ix_should_fail(
        &mut self,
        ix: Instruction,
        signers: &[&Keypair],
    ) {
        self.tx_should_fail(&[ix], signers).await
    }

    pub async fn ix_should_fail_simple(&mut self, ix: Instruction) {
        assert_matches!(self.process_transaction_nonced(&[ix], &[]).await, Err(_));
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
            let lamports = if accounts_exists { self.lamports(&address).await } else { Lamports(100_000) };
            let new_pubkey = Pubkey::new_unique();

            // TODO: owner fuzzing

            let owner = self.context.banks_client.get_account(address).await.unwrap().unwrap().owner;
            self.set_account(&new_pubkey, &data[..], lamports, &owner).await;

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

            let signer = signer.clone();
            self.tx_should_fail(&ixs, &[&signer.keypair]).await;
        }

        let mut ixs = prefix_ixs.to_vec();
        ixs.push(valid_ix);
        self.tx_should_succeed(&ixs, &[&signer.keypair]).await;
    }

    pub async fn genesis_fee(&mut self) -> ProgramFee {
        ProgramFee {
            lamports_per_tx: self.lamports_per_signature().await,
            base_commitment_network_fee: BasisPointFee(11),
            proof_network_fee: BasisPointFee(100),
            base_commitment_subvention: Lamports(33),
            proof_subvention: Lamports(44),
            warden_hash_tx_reward: Lamports(300),
            warden_proof_reward: Lamports(555),
            proof_base_tx_count: (CombinedMillerLoop::TX_COUNT + FinalExponentiation::TX_COUNT + 2) as u64,
        }
    }

    pub async fn set_pda_account<A: SizedAccount + PDAAccount, F>(
        &mut self,
        offset: Option<u32>,
        setup: F,
    )
    where F: Fn(&mut [u8])
    {
        let data_len = A::SIZE;
        let address = A::find(offset).0;
        let mut data = self.data(&address).await;
    
        setup(&mut data);
    
        let rent_exemption = self.rent(data_len).await;
        self.set_program_account(&address, &data, rent_exemption).await;
    }

    pub async fn child_accounts<'a, P: ParentAccount<'a, 'a, 'a> + PDAAccount>(&mut self, data: &'a mut [u8]) -> Vec<Pubkey> {
        let parent = P::new(data).unwrap();
        (0..P::COUNT).map(|i| parent.get_child_pubkey(i).unwrap()).collect()
    }

    pub async fn storage_accounts(&mut self) -> Vec<Pubkey> {
        let mut data = self.data(&StorageAccount::find(None).0).await;
        self.child_accounts::<StorageAccount>(&mut data).await
    }

    pub async fn nullifier_accounts(&mut self, mt_index: u32) -> Vec<Pubkey> {
        let mut data = self.data(&NullifierAccount::find(Some(mt_index)).0).await;
        self.child_accounts::<NullifierAccount>(&mut data).await
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
            test.airdrop(&account, Token::new_checked(token_id, amount).unwrap()).await;
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

/// mut? $id: ident, $ty: ty, $offset: expr, $test: ident
macro_rules! pda_account {
    ($id: ident, $ty: ty, $offset: expr, $test: expr) => {
        pda_account!(data data, $ty, $offset, $test);
        let $id = <$ty>::new(&mut data).unwrap();
    };
    (mut $id: ident, $ty: ty, $offset: expr, $test: expr) => {
        pda_account!(data data, $ty, $offset, $test);
        let mut $id = <$ty>::new(&mut data).unwrap();
    };

    (data $data: ident, $ty: ty, $offset: expr, $test: expr) => {
        let pk = <$ty>::find($offset).0;
        let mut $data = &mut $test.data(&pk).await[..];
    };
}

macro_rules! commitment_queue {
    ($id: ident, $test: expr) => {
        pda_account!(mut q, CommitmentQueueAccount, None, $test);
        let $id = CommitmentQueue::new(&mut q);
    };
    (mut $id: ident, $data: expr) => {
        let mut q = CommitmentQueueAccount::new($data).unwrap();
        let mut $id = CommitmentQueue::new(&mut q);
    };
}

#[allow(unused_imports)] pub(crate) use pda_account;
#[allow(unused_imports)] pub(crate) use commitment_queue;

/// `$ty: ty, $offset: expr, $test: expr, $setup: expr`
macro_rules! set_single_pda_account {
    ($ty: ty, $offset: expr, $test: ident, $setup: expr) => {
        $test.set_pda_account::<$ty, _>($offset, |data| {
            let mut account = <$ty>::new(data).unwrap();
            $setup(&mut account);
        }).await;
    };
}

#[allow(unused_imports)] pub(crate) use set_single_pda_account;

macro_rules! parent_account {
    ($id: ident, $ty: ty) => {
        pub async fn $id<F>(
            pda_offset: elusiv_types::PDAOffset,
            test: &mut ElusivProgramTest,
            f: F,
        ) where F: Fn(&$ty) {
            let mut data = test.data(&<$ty as elusiv_types::PDAAccount>::find(pda_offset).0).await;
            let keys = test.child_accounts::<$ty>(&mut data).await;
        
            let mut v = vec![];
            for &key in keys.iter() {
                let account = test.context().banks_client.get_account(key).await.unwrap().unwrap();
                v.push(account);
            }
        
            let accs = v.iter_mut();
            let mut child_accounts = Vec::new();
            use solana_program::account_info::Account;

            for (i, a) in accs.enumerate() {
                let (lamports, d, owner, executable, epoch) = a.get();
                let child_account = solana_program::account_info::AccountInfo::new(&keys[i], false, false, lamports, d, owner, executable, epoch);
                child_accounts.push(child_account);
            }
        
            let account = <$ty as elusiv_types::accounts::ParentAccount>::new_with_child_accounts(
                &mut data,
                child_accounts.iter().map(|x| Some(x)).collect()
            ).unwrap();

            f(&account)
        }
    };
}

parent_account!(storage_account, StorageAccount);
parent_account!(nullifier_account, NullifierAccount);

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

pub fn u256_from_str(str: &str) -> U256 {
    fr_to_u256_le(&ark_bn254::Fr::from_str(str).unwrap())
}

pub fn u256_from_str_skip_mr(str: &str) -> U256 {
    let n = num::BigUint::from_str(str).unwrap();
    let bytes = n.to_bytes_le();
    let mut result = [0; 32];
    for i in 0..32 {
        if i < bytes.len() {
            result[i] = bytes[i];
        }
    }
    result
}