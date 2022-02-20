use {
    assert_matches::*,
    solana_program::{
        instruction::Instruction,
        instruction::AccountMeta,
        hash::Hash,
        system_program,
        native_token::LAMPORTS_PER_SOL,
    },
    solana_program_test::*,
    solana_sdk::{
        signature::Signer,
        transaction::Transaction,
        signature::Keypair,
    },
    rand::Rng,
    ark_ff::*,

    elusiv::state::*,
    elusiv::fields::scalar::*,
    elusiv::poseidon,
    elusiv::state::TREE_HEIGHT,

    super::accounts::*,
};

pub const DEPOSIT_INSTRUCTIONS_COUNT: u64 = (elusiv::poseidon::ITERATIONS * TREE_HEIGHT + 2) as u64;

pub async fn send_deposit_transaction(payer: &Keypair, recent_blockhash: Hash, data: Vec<u8>) -> Transaction {
    // Start deposit
    let mut instructions = Vec::new();
    instructions.push(Instruction {
        program_id: elusiv::id(),
        accounts: vec!
        [
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(program_account_id(), false),
            AccountMeta::new(deposit_account_id(), false),
        ],
        data: data,
    });

    // Calculate deposit
    for _ in 0..poseidon::ITERATIONS * TREE_HEIGHT - 1 {
        instructions.push(Instruction {
            program_id: elusiv::id(),
            accounts: vec![AccountMeta::new(deposit_account_id(), false)],
            data: vec![elusiv::instruction::COMPUTE_DEPOSIT],
        });
    }

    // Finalize deposit
    instructions.push(Instruction {
        program_id: elusiv::id(),
        accounts: vec!
        [
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(program_account_id(), false),
            AccountMeta::new(deposit_account_id(), false),
            AccountMeta::new(system_program::id(), false),
        ],
        data: vec![elusiv::instruction::FINISH_DEPOSIT],
    });

    // Sign and send transaction
    let mut transaction = Transaction::new_with_payer(&instructions, Some(&payer.pubkey()));
    transaction.sign(&[payer], recent_blockhash);
    transaction
}

pub async fn send_valid_deposit(payer: &Keypair, banks_client: &mut BanksClient, recent_blockhash: Hash) -> Scalar {
    let commitment = valid_commitment();
    let data = deposit_data(commitment);

    let t = send_deposit_transaction(&payer, recent_blockhash, data).await;
    assert_matches!(banks_client.process_transaction(t).await, Ok(()));

    commitment
}

pub fn deposit_data(commitment: Scalar) -> Vec<u8> {
    let mut data = vec![elusiv::instruction::INIT_DEPOSIT];
    let amount: u64 = LAMPORTS_PER_SOL;
    data.extend_from_slice(&amount.to_le_bytes());
    data.extend_from_slice(&to_bytes_le_mont(commitment));
    data
}

// Commitment generation and fetching
fn random_scalar() -> Scalar {
    let mut random = rand::thread_rng().gen::<[u8; 32]>();
    random[31] = 0;
    from_bytes_le_repr(&random).unwrap()
}

pub fn valid_commitment() -> Scalar {
    let nullifier = random_scalar();
    let random = random_scalar();
    poseidon::Poseidon2::new().full_hash(nullifier, random)
}

pub fn get_commitments(account_data: &mut [u8]) -> Vec<Scalar> {
    let tree_leaves = &account_data[TREE_LEAF_START * 32..TREE_SIZE];
    let mut leaves = Vec::new();
    for l in 0..TREE_LEAF_COUNT {
        leaves.push(from_bytes_le_mont(&tree_leaves[l * 32..(l + 1) * 32]))
    }
    leaves
}