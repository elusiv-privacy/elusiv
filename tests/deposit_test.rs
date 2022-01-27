mod common;

use {
    assert_matches::*,
    solana_program_test::*,
    solana_program::{
        native_token::LAMPORTS_PER_SOL,
        instruction::AccountMeta,
        instruction::Instruction,
        system_program,
    },
    solana_sdk::{
        signature::Signer,
        transaction::Transaction,
    },
    elusiv::state::StorageAccount,
    elusiv::state::TOTAL_SIZE,
    elusiv::state::TREE_HEIGHT,
    elusiv::merkle::node,
    elusiv::poseidon::{
        ITERATIONS,
        Scalar,
        from_str_10,
        to_bytes_le_repr,
    },
    common::*,
    ark_ff::*,
};

#[tokio::test]
/// Tests that the finished hash values are corectly stored using the finanilze_deposit instruction
async fn test_deposit_finalize() {
    // Calculated values (first one is the commitment)
    let hashes: [Scalar; TREE_HEIGHT + 1] = [
        from_str_10("1"),
        from_str_10("2"),
        from_str_10("3"),
        from_str_10("4"),
        from_str_10("5"),
        from_str_10("6"),
        from_str_10("7"),
        from_str_10("8"),
        from_str_10("9"),
        from_str_10("10"),
        from_str_10("11"),
        from_str_10("12"),
        from_str_10("13"),
    ];

    // Create program account with hash data
    let setup = |test: &mut ProgramTest| {
        let mut data: Vec<u8> = vec![0; TOTAL_SIZE];
        let mut storage = StorageAccount::from(&mut data).unwrap();
        storage.set_committed_amount(LAMPORTS_PER_SOL);
        storage.set_current_hash_iteration(ITERATIONS as u16);
        storage.set_current_hash_tree_position(TREE_HEIGHT as u16);
        for (i, &hash) in hashes.iter().enumerate() {
            storage.set_finished_hash(i, hash);
        }
        storage.set_hashing_state([*hashes.last().unwrap(), Scalar::zero(), Scalar::zero()]);

        let data = base64::encode(&data);
        test.add_account_with_base64_data(storage_id(), 100000000, elusiv::id(), &data);
    };
    let (mut banks_client, payer, recent_blockhash) = start_program(setup).await;

    // Send finalize_deposit transaction
    let mut transaction = Transaction::new_with_payer(
        &[Instruction {
            program_id: elusiv::id(),
            accounts: vec!
            [
                AccountMeta::new(storage_id(), false),
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(system_program::id(), false),
            ],
            data: vec![2],
        }],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer], recent_blockhash);
    assert_matches!(banks_client.process_transaction(transaction).await, Ok(()));

    // Check that commitment and hashes are saved
    let mut data = get_storage_data(&mut banks_client).await;
    let storage = StorageAccount::from(&mut data).unwrap();
    for (i, &hash) in hashes.iter().enumerate() {
        let node = node(&storage.merkle_tree, TREE_HEIGHT - i, 0);
        println!("{} {} == {}", i, node, hash);
        assert_eq!(
            node,
            hash
        );
    }
}

#[tokio::test]
async fn test_full_deposit() {
    let commitment = from_str_10("244717386276344062509703350126374528606984111509041278484910414242901923926");

    // Create program account
    let setup = |test: &mut ProgramTest| {
        let data: Vec<u8> = vec![0; TOTAL_SIZE];
        let data = base64::encode(&data);
        test.add_account_with_base64_data(storage_id(), 100000000, elusiv::id(), &data);
    };
    let (mut banks_client, payer, recent_blockhash) = start_program(setup).await;

    // Start deposit
    let amount = LAMPORTS_PER_SOL;
    let mut data = vec![0];
    for byte in amount.to_le_bytes() { data.push(byte); }
    for byte in to_bytes_le_repr(commitment) { data.push(byte); }
    let transaction = send_deposit_transaction(storage_id(), &payer, recent_blockhash, data).await;
    assert_matches!(banks_client.process_transaction(transaction).await, Ok(()));

    // Check that the correct values are in the storage
    let expected = [
        commitment,
        from_str_10("21077028657267664303289436742623773635801525100393235099987334295920671060386"),
        from_str_10("6401714785911483142711409338577818817730941624683335634487835789692495342021"),
        from_str_10("18107103040641280716079702331696336397857071925441671391493039726516652555203"),
        from_str_10("18783216614784623605235675043821408613400542945491743609271073508103827961554"),
        from_str_10("20873491173767444317180347865941472669431410129125988069286785196820020269399"),
        from_str_10("9446808616184771799964474536929525461620586514829229704079976701711148559707"),
        from_str_10("2030298405856924199183833350053742355426309321807047354656607137578093773934"),
        from_str_10("11964346641534382871972364579401992885802603630452129146743939828082546785910"),
        from_str_10("9024399474338309616570882172658618435886453158125518251055192212471285919624"),
        from_str_10("21413790189171351969190983614989919390719837053433093795773229302462356308164"),
        from_str_10("3989299271092868442847094777985016599066028468903109025219359254101145990789"),
        from_str_10("5624838273495382817818151473383436058544590187524286868849209100849522715500"),
    ];
    let mut data = get_storage_data(&mut banks_client).await;
    let storage = StorageAccount::from(&mut data).unwrap();
    for i in 0..=TREE_HEIGHT {
        let node = node(&storage.merkle_tree, TREE_HEIGHT - i, 0);
        let expected = expected[i];
        println!("{} {} == {}", i, node, expected);
        assert_eq!(node, expected);
    }

    /*let mut transaction = Transaction::new_with_payer(
        &[Instruction {
            program_id: elusiv::id(),
            accounts: vec!
            [ AccountMeta::new(storage_id(), false) ],
            data: vec![4],
        }],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer], recent_blockhash);
    assert_matches!(banks_client.process_transaction(transaction).await, Ok(()));*/
}