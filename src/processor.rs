use super::instruction::ElusivInstruction;
use super::instruction::ElusivInstruction::*;
use super::error::ElusivError::{
    SenderIsNotSigner,
    InvalidAmount,
    InvalidProof,
    CouldNotProcessProof,
    InvalidMerkleRoot,
    InvalidStorageAccount,
    DidNotFinishHashing,
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
use ark_bn254::*;
use ark_ff::*;
use poseidon::ScalarLimbs;
use poseidon::Scalar;
use poseidon::Poseidon2;

use super::verifier;
use super::state::StorageAccount;

pub struct Processor;

impl Processor {
    pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], instruction: ElusivInstruction) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();

        // 0. [signer] Signer
        let sender = next_account_info(account_info_iter)?;
        if !sender.is_signer { return Err(SenderIsNotSigner.into()); }

        // 1. [owned, writable] Program main account
        // TODO: Add program id verification
        let program_account = next_account_info(account_info_iter)?;
        if program_account.owner != program_id { return Err(InvalidStorageAccount.into()); }
        let data = &mut program_account.data.borrow_mut()[..];
        let mut storage = StorageAccount::from(data)?;

        match instruction {
            InitDeposit { amount, commitment } =>  {
                Self::init_deposit(&mut storage, amount, commitment)
            },
            ComputeDeposit => {
                Self::compute_tree(&mut storage)
            },
            FinishDeposit => {
                // 2. [] System program
                let system_program = next_account_info(account_info_iter)?;
                if *system_program.key != system_program::id() { return Err(IncorrectProgramId); }

                Self::finish_deposit(program_id, program_account, sender, system_program)
            },
            Withdraw { amount, proof, nullifier_hash, root } => {
                // 2. [writable] Recipient
                let recipient = next_account_info(account_info_iter)?;
                if !recipient.is_writable { return Err(InvalidAccountData); }

                Self::withdraw(program_account, recipient, &mut storage, amount, proof, nullifier_hash, root)
            }
        }
    }

    /// Starts the deposit and calculates the first hash iteration
    fn init_deposit(
        storage: &mut StorageAccount,
        amount: u64,
        commitment: ScalarLimbs
    ) -> ProgramResult {
        // Check amount
        if amount != LAMPORTS_PER_SOL { return Err(InvalidAmount.into()); }

        // Check commitment
        storage.can_insert_commitment(commitment)?;

        // Reset values
        storage.set_committed_amount(amount);
        storage.set_current_hash_iteration(poseidon::ITERATIONS as u16);
        storage.set_current_hash_tree_position(0);

        // Add commitment to hashing state and finished hash store
        let commitment = poseidon::from_limbs(&commitment);
        storage.set_finished_hash(0, commitment);
        storage.set_hashing_state([commitment, Scalar::zero(), Scalar::zero()]);

        // Start first hash
        Self::compute_tree(storage)
    }

    /// Calculates the hash iterations
    fn compute_tree(storage: &mut StorageAccount) -> ProgramResult {
        // Fetch values
        let mut current_tree_position = storage.get_current_hash_tree_position();
        let mut current_iteration = storage.get_current_hash_iteration();
        let mut state = storage.get_hashing_state();

        // Move to next tree level or finish 
        if current_iteration as usize == poseidon::ITERATIONS {
            // Save hash
            let previous_hash = state[0];
            storage.set_finished_hash(current_tree_position as usize, previous_hash);

            // Reset values
            let index = storage.leaf_pointer();
            let neighbour = super::merkle::get_neighbour(&storage.merkle_tree, current_tree_position as usize, index as usize);
            let last_hash_is_left = ((index >> current_tree_position) & 1) == 0;
            current_tree_position += 1;
            current_iteration = 0;

            // Set new inputs
            state[0] = Scalar::zero();
            state[1] = if last_hash_is_left { previous_hash } else { neighbour };
            state[2] = if last_hash_is_left { neighbour } else { previous_hash };

            // Finished
            if current_tree_position as usize > super::state::TREE_HEIGHT { return Ok(()) }
        }

        // Hash
        let hash = Poseidon2::new().partial_hash(current_iteration as usize, state[0], state[1], state[2]);
        storage.set_hashing_state(hash);

        // Save values
        current_iteration += 1;
        storage.set_current_hash_iteration(current_iteration);
        storage.set_current_hash_tree_position(current_tree_position);

        Ok(())
    }

    /// Runs the last hash iteration and stores the commitment and hash values
    fn finish_deposit<'a>(
        program_id: &Pubkey,
        program_account: & AccountInfo<'a>,
        sender: & AccountInfo<'a>,
        system_program: & AccountInfo<'a>,
    ) -> ProgramResult {
        let amount;

        {
            let data = &mut program_account.data.borrow_mut()[..];
            let mut storage = StorageAccount::from(data)?;

            // Compute last hash iteration
            Self::compute_tree(&mut storage)?;

            // Check if hashing is finished
            if storage.get_current_hash_iteration() != 0 || (storage.get_current_hash_tree_position() as usize) <= super::state::TREE_HEIGHT {
                return Err(DidNotFinishHashing.into())
            }

            // Fetch the amount
            amount = storage.get_committed_amount();

            // Save the commitment and calculated values in the merkle tree
            storage.add_commitment()?;
        }

        // Transfer funds using system program
        let instruction = transfer(&sender.key, program_account.key, amount);
        let (_, bump_seed) = Pubkey::find_program_address(&[b"deposit"], program_id);
        invoke_signed(
            &instruction,
            &[
                sender.clone(),
                program_account.clone(),
                system_program.clone(),
            ],
            &[&[&b"deposit"[..], &[bump_seed]]],
        )?;

        Ok(())
    }

    fn withdraw(
        program_account: &AccountInfo,
        recipient: &AccountInfo,
        storage: &mut StorageAccount,
        amount: u64,
        proof: Proof<Bn254>,
        nullifier_hash: ScalarLimbs,
        root: ScalarLimbs,
    ) -> ProgramResult {
        // Check the amount
        if amount != LAMPORTS_PER_SOL { return Err(InvalidAmount.into()); }

        // Check if nullifier does not already exist
        storage.can_insert_nullifier_hash(nullifier_hash)?;

        // Check merkle root
        if !storage.is_root_valid(root) { return Err(InvalidMerkleRoot.into()) }

        // Validate proof
        let pvk = prepare_verifying_key(&verifier::verification_key());
        let inputs: Vec<Scalar> = vec![
            poseidon::from_limbs(&root),
            poseidon::from_limbs(&nullifier_hash),
        ];
        let result = verify_proof(&pvk, &proof, &inputs[..]);
        match result {
            Ok(verified) => if !verified { return Err(InvalidProof.into()); },
            Err(_) => return Err(CouldNotProcessProof.into())
        }

        // Save nullifier
        storage.insert_nullifier_hash(nullifier_hash)?;

        // Transfer funds using owned bank account
        **program_account.try_borrow_mut_lamports()? -= amount;
        **recipient.try_borrow_mut_lamports()? += amount;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use poseidon::{
        from_bytes_le,
        from_str_16,
        bytes_to_limbs,
        to_bytes_le,
        to_hex_string
    };
    use super::super::merkle::{
        get_node,
        insert_hashes,
        initialize_store,
    };
    use super::super::state::TREE_HEIGHT;

    fn compute_full_merkle_tree(commitment: &str, root: &str, intermediary: Option<[&str; TREE_HEIGHT - 1]>) {
        // Initialize Merkle tree with hashes of default value
        let mut data = [0 as u8; super::super::state::TOTAL_SIZE];
        let mut storage = StorageAccount::from(&mut data).unwrap();
        let hasher = poseidon::Poseidon2::new();
        let hash = |left: Scalar, right: Scalar| { hasher.full_hash(left, right) };
        initialize_store(&mut storage.merkle_tree, Scalar::zero(), hash);
        let commitment = bytes_to_limbs(&to_bytes_le(from_str_16(commitment).unwrap()));

        // Init deposit
        Processor::init_deposit(&mut storage, LAMPORTS_PER_SOL, commitment).unwrap();

        // Computation
        for _ in 0..poseidon::ITERATIONS * (TREE_HEIGHT + 1) - 1 {
            Processor::compute_tree(&mut storage).unwrap();
        }
        
        // Complete by manually saving the data
        let hashes = storage.get_finished_hashes_storage();
        let leaf_index = storage.leaf_pointer() as usize;
        insert_hashes(&mut storage.merkle_tree, hashes, leaf_index);

        // Check intermediary hashes
        /*if let Some(intermediary) = intermediary {
            for (i, bytes in hashes.iter().skip(1) {
                assert_eq!(from_str_16(root).unwrap(), 
                let value = from_bytes_le(&bytes);
                println!("{}", to_hex_string(value));
            }
        }*/

        assert_eq!(from_str_16(root).unwrap(), get_node(&storage.merkle_tree, 0, 0))
    }

    #[test]
    fn test_full_merkle_computation() {
        compute_full_merkle_tree("0x201C9EE36252934C7C54843A0D47ADAA14102E63EED90EC080358C5F7BAFBAF8", "0x0BF672DB38CF8B4DC8BF212269A72972E1386C0270D1C37ACBADD856BF7E4F17", None);
        compute_full_merkle_tree("0x20A67EA684881990392E60B9CFB67DD389C55BEACB171D41DD8ED26E4DC95366", "0x0BD26416A97B03FDE524D90341A8B2642E4CAB56C540B4267F2CDC7F93D8B2F8", None);
        compute_full_merkle_tree("0x306184A154572C6025FF8CA6010A4575F914BAE9739D467EFC11F4BCA3611CDF", "0x2E01B47008405889E5A256D5760E01468EDC5936FEE389656399107293D98B95", None);
        compute_full_merkle_tree("0x1998702D852608250CF3FE516A428D3C0173509B0884A03A919E45D2EEA7AA0E", "0x0C6122A7A6AAC76C59C10251110486EBD09E9C7F021FD4146E068356EF24B9EF", None);
        compute_full_merkle_tree("0x11B72B69B816650B853B3A3E3A1260F08D0051B1742F165F48A01D4082A21260", "0x19FCB4FB8392F4BDDAF4F2EEAD29A8777FEC5461B0DF6C3FF4B7F4DC97A94AE9", None);
        compute_full_merkle_tree("0x11C3A67A32FA85BE319938A1AB786875FDC78BF35076D3F580577FE7107B64FB", "0x1DB54339FA57D77C1A99474178BB2F4D32C11F8ED89794F61DB215384CE7EB0B", None);
        compute_full_merkle_tree("0x0CC7E3DF9F18212DFCCB695FD5944D4FA838A64530D1EB51B9E1E61FFD436413", "0x2CD4C0391844FA35459B02AD2BB0E1C4566CE9E3179A7967241F9C8443ADF7D6", None);
        compute_full_merkle_tree("0x1BCF981642D1D02CD8586F1CEA19A3B3562CC91C2EFBF2AA26B18A0DC20C8164", "0x2553D6AB70B0A91A667267E821AADA066F340B6CEA046C45B42CCAF9434CF8E7", None);
    }

    #[test]
    #[should_panic]
    #[ignore]
    fn test_full_merkle_computation_false() {
        compute_full_merkle_tree("0x201C9EE36252934C7C54843A0D47ADAA14102E63EED90EC080358C5F7BAFBAF8", "0x0BF672DB38CF8B4DC8BF212269A72972E1386C0270D1C37ACBADD856BF7E4F18", None);
        compute_full_merkle_tree("0x1BCF981642D1D02CD8586F1CEA19A3B3562CC91C2EFBF2AA26B18A0DC20C8164", "0x2553D6AB70B0A91A667267E821AADA066F340B6CEA046C45B42CCAF9434CF8F7", None);
        compute_full_merkle_tree("0x1C8282B9AE1E51CCB6D73160D8ED5D7B85F958390DC33BE5C1B5EEE04E6B6198", "0x08AF92BEDC66FDA2D9BC40CED6038448E536E660954BB104C02B8A123478B0E9", None);
        compute_full_merkle_tree("0x2084B29FC35D1EBAEF8C7FA6655594B977020CA0506C8D40D8629B6E9AD5222F", "0x2CA4B3F41656B597CCD4B6549E121C7ED5640D7D8BDF758325081057C587949E", None);
    }

    #[test]
    fn test_full_merkle_computation_with_intermediary() {

    }

    #[test]
    fn test_computation_does_not_affect_storage() {

    }
}