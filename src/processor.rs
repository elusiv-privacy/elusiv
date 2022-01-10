use super::instruction::{
    ElusivInstruction,
    ElusivInstruction::Deposit,
    ElusivInstruction::Withdraw,
};
use super::error::ElusivError::{
    SenderIsNotSigner,
    SenderIsNotWritable,
    InvalidAmount,
    InvalidProof,
    CouldNotProcessProof,
    InvalidMerkleRoot,
    InvalidStorageAccount,
};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    program_error::ProgramError::{
        InvalidAccountData,
        IncorrectProgramId,
    },
    account_info::next_account_info,
    system_instruction::transfer,
    program::invoke_signed,
    system_program,
    native_token::LAMPORTS_PER_SOL,
};
use ark_groth16::{
    Proof,
    verify_proof,
    prepare_verifying_key,
};
use ark_bn254::Bn254;
use poseidon::scalar::ScalarLimbs;
use poseidon::scalar::Scalar;
use poseidon::scalar;

use super::verifier;
use super::state::StorageAccount;

pub struct Processor;

impl Processor {
    pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], instruction: ElusivInstruction) -> ProgramResult {
        match instruction {
            Deposit { amount, commitment } =>  {
                Self::deposit(program_id, &accounts, amount, commitment)
            },
            Withdraw { amount, proof, nullifier_hash, root } => {
                Self::withdraw(program_id, &accounts, amount, proof, nullifier_hash, root)
            }
        }
    }

    fn deposit(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        amount: u64,
        commitment: ScalarLimbs,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();

        // 0. [signer, writable] Signer and Sender
        let sender = next_account_info(account_info_iter)?;
        if !sender.is_signer { return Err(SenderIsNotSigner.into()); }
        if !sender.is_writable { return Err(SenderIsNotWritable.into()); }

        // 1. [owned, writable] Program main account
        // TODO: Not enough to check for ownership, account needs specific address
        let bank = next_account_info(account_info_iter)?;
        if bank.owner != program_id { return Err(InvalidStorageAccount.into()); }

        // 2. System program
        let system_program = next_account_info(account_info_iter)?;
        if *system_program.key != system_program::id() { return Err(IncorrectProgramId); }

        // Check the amount
        if amount != LAMPORTS_PER_SOL { return Err(InvalidAmount.into()); }

        {
            let data = &mut bank.data.borrow_mut()[..];
            let mut storage = StorageAccount::from(data)?;

            // Check if commitment is unique and insert commitment in merkle tree
            storage.try_add_commitment(commitment)?;
        }

        // Transfer funds using system program
        let instruction = transfer(&sender.key, &bank.key, amount);
        let (_, bump_seed) = Pubkey::find_program_address(&[b"deposit"], program_id);
        invoke_signed(
            &instruction,
            &[
                sender.clone(),
                bank.clone(),
                system_program.clone(),
            ],
            &[&[&b"deposit"[..], &[bump_seed]]],
        )?;

        Ok(())
    }

    fn withdraw(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        amount: u64,
        proof: Proof<Bn254>,
        nullifier_hash: ScalarLimbs,
        root: ScalarLimbs,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();

        // 0. [signer] Signer
        let sender = next_account_info(account_info_iter)?;
        if !sender.is_signer { return Err(SenderIsNotSigner.into()); }

        // 1. [owned, writable] Program main account
        let bank = next_account_info(account_info_iter)?;
        if bank.owner != program_id { return Err(InvalidStorageAccount.into()); }
        let data = &mut bank.data.borrow_mut()[..];
        let mut storage = StorageAccount::from(data)?;

        // 2. [writable] Recipient
        let recipient = next_account_info(account_info_iter)?;
        if !sender.is_writable { return Err(InvalidAccountData); }

        // Check the amount
        if amount != LAMPORTS_PER_SOL { return Err(InvalidAmount.into()); }

        // Check if nullifier does not already exist
        storage.can_insert_nullifier_hash(nullifier_hash)?;

        // Check merkle root
        if !storage.is_root_valid(root) { return Err(InvalidMerkleRoot.into()) }

        // Validate proof
        let pvk = prepare_verifying_key(&verifier::verification_key());
        let inputs: Vec<Scalar> = vec![
            scalar::from_limbs(&root),
            scalar::from_limbs(&nullifier_hash),
        ];
        let result = verify_proof(&pvk, &proof, &inputs[..]);
        match result {
            Ok(verified) => if !verified { return Err(InvalidProof.into()); },
            Err(_) => return Err(CouldNotProcessProof.into())
        }

        // Save nullifier
        storage.insert_nullifier_hash(nullifier_hash)?;

        // Transfer funds using owned bank account
        **bank.try_borrow_mut_lamports()? -= amount;
        **recipient.try_borrow_mut_lamports()? += amount;

        Ok(())
    }
}