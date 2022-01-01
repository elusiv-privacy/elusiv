import {
    createKeypairFromFile,
    getPayer
} from './utils';
import {
    PublicKey,
    Transaction,
    SystemProgram,
    sendAndConfirmTransaction
} from '@solana/web3.js';
import {
    establish_connection
} from 'elusiv_client';

export async function setup_program_account(program_id: PublicKey, size: number): Promise<PublicKey> {
    const payer = await getPayer();
    const connection = await establish_connection();
    const seed = size.toString();
    const account_pubkey = await PublicKey.createWithSeed(payer.publicKey, seed, program_id);

    if (await connection.getAccountInfo(account_pubkey) != null) {
        return account_pubkey;
    }

    const rent = await connection.getMinimumBalanceForRentExemption(100);

    // Airdrop the fee_payer sufficient funds
    const { feeCalculator } = await connection.getRecentBlockhash();
    const minimum_lamport = rent + feeCalculator.lamportsPerSignature * 100;
    const balance = await connection.getBalance(payer.publicKey);
    if (balance < minimum_lamport) {
        const airdrop = await connection.requestAirdrop(payer.publicKey, minimum_lamport - balance);
        await connection.confirmTransaction(airdrop);
    }

    // Create new account
    const setup_transaction = new Transaction().add(
        SystemProgram.createAccountWithSeed({
            fromPubkey: payer.publicKey,
            basePubkey: payer.publicKey,
            seed: seed,
            newAccountPubkey: account_pubkey,
            lamports: rent,
            space: size,
            programId: program_id,
        }),
    );
    await sendAndConfirmTransaction(connection, setup_transaction, [payer]);

    return account_pubkey;
}