mod common;
use {
    assert_matches::*,
    solana_program_test::*,
    solana_sdk::signature::Signer,
    ark_bn254::{
        G1Projective,
        G1Affine
    },
    ark_ec::{
        AffineCurve,
        ProjectiveCurve,
    },
    elusiv::state::ProofVerificationAccount,
    elusiv::scalar::*,
    common::*,
};

#[tokio::test]
async fn test_prepare_inputs() {
    // Check that gamma_abc_g1 match in the verifying keys
    assert_eq!(
        elusiv::groth16::gamma_abc_g1_0(),
        elusiv::groth16::gamma_abc_g1()[0].into_projective()
    );

    // Setup program and storage account
    let (mut banks_client, payer, recent_blockhash) = start_program_with_program_accounts().await;

    // Withdrawal data
    let recipient = payer.pubkey();
    let proof = ProofString {
        ax: "20126663690791185061571811803880660400249664041725752506932891761257080457600",
        ay: "8576898640855193434532957058024622754523601935685710982895989078519420295236",
        az: "1",

        bx0: "10955106982148508822335779978464475302901082442383559595900533966486935412860",
        bx1: "14050489335097178220473822201837563496022784893028304178309668805697058273427",
        by0: "18797925105295898087664279579148117049337272609674797102042861321566805095703",
        by1: "5740915918963877411701941112629152880674732534983232020073738816072255979877",
        bz0: "1",
        bz1: "0",

        cx: "11394983153127930608194421494611415508023030266359925229388579550239449321375",
        cy: "16282616176077653959710302702422273062672284963512759153470996569152357979675",
        cz: "1",
    };

    let inputs = [
        "20643720223837027367320733428836459266646763523911772324593310161284187566894",
        "19526707366532583397322534596786476145393586591811230548888354920504818678603"
    ];

    // Send transaction
    let t = withdraw_transaction(&payer, recipient, recent_blockhash, withdraw_data(proof, &inputs)).await;
    assert_matches!(banks_client.process_transaction(t).await, Ok(()));

    // Check if prepared_inputs match
    let mut storage = get_account_data(&mut banks_client, withdraw_account_id()).await;
    let account = ProofVerificationAccount::from_data(&mut storage).unwrap();
    let prepared_inputs = read_g1_projective(&account.p_inputs);
    
    let pvk = ark_pvk();
    let inputs = vec![
        from_str_10(inputs[0]),
        from_str_10(inputs[1]),
    ];
    let expect = ark_groth16::prepare_inputs(&pvk, &inputs).unwrap();

    println!("a: {}", prepared_inputs);
    println!("b: {}", expect);
    assert_eq!(
        prepared_inputs,
        expect
    );
}