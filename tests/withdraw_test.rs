/*mod common;

use {
    assert_matches::*,
    solana_program_test::*,
    solana_sdk::signature::Signer,
    poseidon::*,
    solana_program::native_token::LAMPORTS_PER_SOL,
};
use common::*;
use ark_ff::*;
*/

/*fn withdraw_data(proof: ProofString, inputs: &[&str]) -> Vec<u8> {
    let mut data = vec![1];

    let amount: u64 = LAMPORTS_PER_SOL;
    data.extend_from_slice(&amount.to_le_bytes());

    proof.push_to_vec(&mut data);

    for input in inputs {
        data.extend(str_to_bytes(input));
    }

    data
}*/

/*#[tokio::test]
async fn test_withdraw() {
    // Setup program and storage account
    let (mut banks_client, payer, recent_blockhash) = start_program_with_storage(storage_id()).await;

    let storage_balance = get_balance(&mut banks_client, storage_id()).await;

    // Generate commitment and send deposit
    let nullifier = from_str_16("0x0070FA9884550D4DC0E5084A7534E0D0DC0BCC1AB6D9273B0A9EF1570D1D47EC").unwrap();
    let random = from_str_16("0x00A14157ED0ACC3E60CDF8E946BA124AB5EF8E463700AD7471B037C792EDCADF").unwrap();
    let poseidon = Poseidon2::new();
    let commitment = poseidon.full_hash(nullifier, random);
    assert_eq!(to_hex_string(commitment), "0x2F35A39ADA15DF56E6FE48F24E5FC16C5AB45E2380C1C9C3730B35306184F415");
    let t = send_deposit_transaction(elusiv::id(), storage_id(), &payer, recent_blockhash, deposit_data(commitment)).await;
    assert_matches!(banks_client.process_transaction(t).await, Ok(()));

    assert_eq!(
        storage_balance + LAMPORTS_PER_SOL,
        get_balance(&mut banks_client, storage_id()).await
    );

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
    let t = send_withdraw_transaction(elusiv::id(), storage_id(), payer, recipient, recent_blockhash, withdraw_data(proof, &inputs)).await;
    assert_matches!(banks_client.process_transaction(t).await, Ok(()));

    assert_eq!(
        storage_balance,
        get_balance(&mut banks_client, storage_id()).await
    );
}
*/