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
    ExplicitLogError,
};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    program_error::ProgramError::{
        InvalidAccountData,
        IncorrectProgramId,
        InvalidArgument,
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
use super::poseidon::*;
use super::poseidon;

use super::verifier;
use super::state::StorageAccount;

pub struct Processor;

impl Processor {
    pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], instruction: ElusivInstruction) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();

        // Program Account (check for ownership and correct pubkey)
        let program_account = next_account_info(account_info_iter)?;
        if program_account.owner != program_id { return Err(InvalidStorageAccount.into()); }
        //if program_account.key != &super::state::id() { return Err(InvalidStorageAccount.into()) }

        // ComputeDepoit instruction (does not have signer account)
        if let ComputeDeposit = instruction {
            let data = &mut program_account.data.borrow_mut()[..];
            let mut storage = StorageAccount::from(data)?;

            return Self::compute_deposit(&mut storage);
        }

        // Signer account
        let signer = next_account_info(account_info_iter)?;

        // FinishDeposit (does not use storage_account)
        if let FinishDeposit = instruction {
            // Signer property only relevant for deposit
            if !signer.is_signer { return Err(SenderIsNotSigner.into()); }

            // 2. [] System program
            let system_program = next_account_info(account_info_iter)?;
            if *system_program.key != system_program::id() { return Err(IncorrectProgramId); }

            return Self::finish_deposit(program_id, program_account, signer, system_program);
        }

        let data = &mut program_account.data.borrow_mut()[..];
        let mut storage = StorageAccount::from(data)?;

        match instruction {
            // InitDeposit (called once)
            InitDeposit { amount, commitment } =>  {
                // Signer property only relevant for deposit
                if !signer.is_signer { return Err(SenderIsNotSigner.into()); }
                
                Self::init_deposit(&mut storage, amount, commitment)
            },
            // Withdraw (called once)
            Withdraw { amount, proof, nullifier_hash, root } => {
                // 2. [writable] Recipient
                let recipient = next_account_info(account_info_iter)?;
                if !recipient.is_writable { return Err(InvalidAccountData); }

                Self::withdraw(program_account, recipient, &mut storage, amount, proof, nullifier_hash, root)
            },
            Log { index } => {
                Self::log(&mut storage, index)
            }
            _ => Err(InvalidArgument)
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
        let commitment = from_limbs_mont(&commitment);
        storage.set_finished_hash(0, commitment);
        storage.set_hashing_state([commitment, Scalar::zero(), Scalar::zero()]);

        // Start first hash
        Self::compute_deposit(storage)
    }

    /// Calculates the hash iterations
    fn compute_deposit(storage: &mut StorageAccount) -> ProgramResult {
        // Fetch values
        let mut current_tree_position = storage.get_current_hash_tree_position();
        let mut current_iteration = storage.get_current_hash_iteration();
        let mut state = storage.get_hashing_state();

        // Move to next tree level
        if current_iteration as usize == poseidon::ITERATIONS {
            // Save hash
            let previous_hash = state[0];
            storage.set_finished_hash(current_tree_position as usize, previous_hash);

            // Reset values
            let index = storage.leaf_pointer() >> (current_tree_position as usize);
            let layer = super::state::TREE_HEIGHT - current_tree_position as usize;
            let neighbour = super::merkle::neighbour(&storage.merkle_tree, layer, index as usize);
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

    /// Save the hashes after finishing the deposit hashes
    fn finalize_deposit(storage: &mut StorageAccount) -> ProgramResult {
        let current_tree_position = storage.get_current_hash_tree_position() as usize;
        let current_hash_iteration = storage.get_current_hash_iteration() as usize;

        // Assert that hashing is finished
        if current_hash_iteration != poseidon::ITERATIONS || current_tree_position != super::state::TREE_HEIGHT {
            return Err(DidNotFinishHashing.into())
        }

        // Store last hash
        storage.set_finished_hash(current_tree_position, storage.get_hashing_state()[0]);

        // Store all hashes
        storage.add_commitment()?;

        // Reset the hash process values
        storage.set_current_hash_iteration(poseidon::ITERATIONS as u16);
        storage.set_current_hash_tree_position(0);

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

            // Check if hashing is finished
            // Save the commitment and calculated values in the merkle tree
            Self::finalize_deposit(&mut storage)?;

            // Fetch the amount
            amount = storage.get_committed_amount();
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

    /// Withdraw the amount to the recipient using the proof
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
            from_limbs_mont(&root),
            from_limbs_mont(&nullifier_hash),
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

    fn log(storage: &mut StorageAccount, index: u8) -> ProgramResult {
        let index = index as usize;
        use solana_program::msg;
        use super::state::TREE_HEIGHT;

        msg!(&format!("Nodes above index {} (commitment to root):", index));

        for i in 0..=TREE_HEIGHT {
            let layer = TREE_HEIGHT - i;
            let node = super::merkle::node(&storage.merkle_tree, layer, index >> i);
            msg!(&format!("Layer: {}: {}", layer, node));
        }

        for i in 0..=TREE_HEIGHT {
            let layer = TREE_HEIGHT - i;
            let index = super::merkle::store_index(layer, index >> i);
            let bytes = &storage.merkle_tree[index..index + 32];
            msg!(&format!("Layer: {}: {:?}", layer, bytes));
        }

        Err(ExplicitLogError.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::merkle::{
        node,
        initialize_store,
    };
    use super::super::state::{
        TREE_HEIGHT,
        TOTAL_SIZE,
    };

    fn init_merkle_tree<'a>(index: usize) -> [u8; TOTAL_SIZE] {
        let mut data = [0 as u8; TOTAL_SIZE];
        let mut storage = StorageAccount::from(&mut data).unwrap();
        let hasher = poseidon::Poseidon2::new();
        let hash = |left: Scalar, right: Scalar| { hasher.full_hash(left, right) };
        initialize_store(&mut storage.merkle_tree, Scalar::zero(), hash);
        storage.increment_leaf_pointer(index).unwrap();
        data
    }

    fn test_compute_merkle_tree(commitment: &str, hashes: [(usize, &str); TREE_HEIGHT + 1]) {
        // Init Storage
        let mut data = [0 as u8; TOTAL_SIZE];
        let mut storage = StorageAccount::from(&mut data).unwrap();

        // Init Deposit
        let commitment = bytes_to_limbs(&to_bytes_le_mont(from_str_10(commitment)));
        Processor::init_deposit(&mut storage, LAMPORTS_PER_SOL, commitment).unwrap();

        // Deposit Computation
        for _ in 0..poseidon::ITERATIONS * (TREE_HEIGHT) - 1 {
            Processor::compute_deposit(&mut storage).unwrap();
        }
        
        // Finish Deposit
        Processor::finalize_deposit(&mut storage).unwrap();

        // Check hashes
        for (i, (index, str)) in hashes.iter().enumerate() {
            println!("{}", i);
            assert_eq!(
                from_str_10(str),
                node(&storage.merkle_tree, TREE_HEIGHT - i, *index)
            );
        }
    }

    #[test]
    fn test_full_merkle_computation() {
        test_compute_merkle_tree(
            "8144211214817430829349003215074481182100404296535680119964943950269151541972",
            [
                (0, "8144211214817430829349003215074481182100404296535680119964943950269151541972"),
                (0, "3521277125107847192640759927250026508659373094488056016877049883968245990497"),
                (0, "11008470601289425019669613099340238804865634385945939048577684066343564163598"),
                (0, "4925659323704439400753199337811643084327152060739597663701472281761651058023"),
                (0, "14765708061323393055475531548205222690842122705898817308656304459922553086837"),
                (0, "18302505260897933410141436351606000613718187608915693130840205380439479067929"),
                (0, "15560114830177106954290712716582303036737780625523827404682005490525721812059"),
                (0, "20642714280510803624802215662417179290254187237731073029361060343357312835923"),
                (0, "16587521098739437248802764426632498644914282507827942298132659879495095924825"),
                (0, "4149218072247539680507923870346474049901657161679310495666398415243659094729"),
                (0, "16316872159433614824217699109767005161622149149954579797525053347939642104469"),
                (0, "11107893281581913620018679391305793670957211692198638126634713408321358294960"),
                (0, "7064984100162601639298203620255293723978691319082011519452107984312746295454"),
            ]
        );

        test_compute_merkle_tree(
            "13552763967912093594457579779110052252941986640568606066796890732453878304904",
            [
                (0, "13552763967912093594457579779110052252941986640568606066796890732453878304904"),
                (0, "2788832706231923317949979783323167016733265655607476807262415957398223972822"),
                (0, "3079351413451707819517574021930381150131513959312277116529435787947055097510"),
                (0, "15063597621654239999630569706770040577352868492762243899768935252571212311732"),
                (0, "3293521682234057674324192569598719801866416901412061638617347215370468949294"),
                (0, "11867097728154359806218486819879720367912296696888537396534232707560018988134"),
                (0, "16668769246171179223962940205857965012137568601711016032950906078278091464609"),
                (0, "5713123214686094636857372662971416814262011789711214832145449419665308682592"),
                (0, "15476193130701712784637788523969980650929758210521358989149687329814948954906"),
                (0, "9319319632942327348589534865826491746302422844697651057933979196977281940635"),
                (0, "6661709225075433042433773935071907350729604796398468086432558285509406475644"),
                (0, "20338143476584452910749922184091819440206294559351690339543524003947546518187"),
                (0, "5920793278778744732122613922920389866758626491292944536621920552573424741009"),
            ]
        );
    }

    #[test]
    fn test_compute_multiple_commitments() {
        // Init Storage
        let mut data = [0 as u8; TOTAL_SIZE];
        let mut storage = StorageAccount::from(&mut data).unwrap();

        // Deposit 0
        let commitment = bytes_to_limbs(&to_bytes_le_mont(from_str_10("13742746012751083277892228377764260000239534456878525049335647276801809645457")));
        Processor::init_deposit(&mut storage, LAMPORTS_PER_SOL, commitment).unwrap();
        for _ in 0..poseidon::ITERATIONS * (TREE_HEIGHT) - 1 { Processor::compute_deposit(&mut storage).unwrap(); }
        Processor::finalize_deposit(&mut storage).unwrap();
        assert_eq!(
            node(&storage.merkle_tree, 0, 0),
            from_str_10("12986367982040817160540332433371902400206274668282942567679375016030163683535")
        );

        // Deposit 1
        let commitment = bytes_to_limbs(&to_bytes_le_mont(from_str_10("17598496762772913768842234443529375820067611385012556852766388745086067053344")));
        Processor::init_deposit(&mut storage, LAMPORTS_PER_SOL, commitment).unwrap();
        for _ in 0..poseidon::ITERATIONS * (TREE_HEIGHT) - 1 { Processor::compute_deposit(&mut storage).unwrap(); }
        Processor::finalize_deposit(&mut storage).unwrap();
        let poseidon = Poseidon2::new();

        // Explicit first hash check
        assert_eq!(
            node(&storage.merkle_tree, TREE_HEIGHT - 1, 0),
            from_str_10("9250582908633439312254427073747285879616368085421326049753099391976074597705")
        );

        // Check hashes
        let mut hash = poseidon.full_hash(
            from_str_10("13742746012751083277892228377764260000239534456878525049335647276801809645457"),
            from_str_10("17598496762772913768842234443529375820067611385012556852766388745086067053344")
        );
        for i in 1..=TREE_HEIGHT {
            println!("{}", i);
            assert_eq!(hash, node(&storage.merkle_tree, TREE_HEIGHT - i, 0));
            hash = poseidon.full_hash(hash, Scalar::zero());
        }

        // Explicit root check
        assert_eq!(
            node(&storage.merkle_tree, 0, 0),
            from_str_10("14179231500255854581437255655788433381559803720571568228288219562259757099116")
        );
    }

    #[test]
    fn test_computation_does_not_affect_storage() {
        fn clone_merkle(merkle: &mut [u8]) -> Vec<u8> {
            let mut merk: Vec<u8> = Vec::new();
            for byte in merkle { merk.push(*byte); }
            merk
        }

        // Init Storage
        let mut data = init_merkle_tree(0);
        let mut storage = StorageAccount::from(&mut data).unwrap();
        let original_merkle = clone_merkle(storage.merkle_tree);

        // Init Deposit
        let commitment = bytes_to_limbs(&to_bytes_le_mont(from_str_16("0x121C2AF32EBBAB8932DFCBC77B3A942F5A4E1040EE7157C291131B002F387C00").unwrap()));
        Processor::init_deposit(&mut storage, LAMPORTS_PER_SOL, commitment).unwrap();

        // Deposit Computation
        for _ in 0..poseidon::ITERATIONS * (TREE_HEIGHT + 1) - 2 {
            Processor::compute_deposit(&mut storage).unwrap();
        }

        assert_eq!(original_merkle, clone_merkle(storage.merkle_tree));
    }
}