use crate::fields::utils::vec_to_array_32;

use super::instruction::ElusivInstruction;
use super::instruction::ElusivInstruction::*;
use super::error::ElusivError::{
    SenderIsNotSigner,
    InvalidAmount,
    InvalidRecipient,
    DidNotFinishHashing,
    InvalidProof,
    InvalidMerkleRoot,
};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    program_error::ProgramError::{
        IncorrectProgramId,
        InvalidArgument,
    },
    account_info::next_account_info,
    system_instruction::transfer,
    program::invoke_signed,
    system_program,
    native_token::LAMPORTS_PER_SOL,
};
use ark_ff::Zero;
use super::groth16::{
    PREPARE_INPUTS_ITERATIONS,
    MILLER_LOOP_ITERATIONS,
    FINAL_EXPONENTIATION_ITERATIONS,
    partial_prepare_inputs,
    partial_miller_loop,
    partial_final_exponentiation,
    verify_proof,
    Proof,
};
use super::fields::scalar::*;
use super::poseidon::*;
use super::poseidon;
use super::merkle;

// Storage accounts
use super::state::ProgramAccount;
use super::poseidon::DepositHashingAccount;
use super::groth16::ProofVerificationAccount;

const MINIMUM_AMOUNT: u64 = LAMPORTS_PER_SOL / 10;

pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], instruction: ElusivInstruction) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    match instruction {
        InitDeposit { amount, commitment } => {
            // Signer account
            let signer = next_account_info(account_info_iter)?;
            if !signer.is_signer { return Err(SenderIsNotSigner.into()); }

            // Program account
            let program_account = next_account_info(account_info_iter)?;
            let data = &mut program_account.data.borrow_mut()[..];
            let mut program_account = ProgramAccount::new(&program_account, data, program_id)?;

            // Deposit account
            let deposit_account = next_account_info(account_info_iter)?;
            let data = &mut deposit_account.data.borrow_mut()[..];
            let mut deposit_account = DepositHashingAccount::new(&deposit_account, data, program_id)?;

            init_deposit(&mut program_account, &mut deposit_account, amount, commitment)
        },
        ComputeDeposit => {
            // Deposit account
            let deposit_account = next_account_info(account_info_iter)?;
            let data = &mut deposit_account.data.borrow_mut()[..];
            let mut deposit_account = DepositHashingAccount::new(&deposit_account, data, program_id)?;

            compute_deposit(&mut deposit_account)
        },
        FinishDeposit => {
            // Signer account
            let signer = next_account_info(account_info_iter)?;
            if !signer.is_signer { return Err(SenderIsNotSigner.into()); }

            // Program account
            let program_account = next_account_info(account_info_iter)?;

            // Deposit account
            let deposit_account = next_account_info(account_info_iter)?;
            let data = &mut deposit_account.data.borrow_mut()[..];
            let mut deposit_account = DepositHashingAccount::new(&deposit_account, data, program_id)?;

            // System program
            let system_program = next_account_info(account_info_iter)?;
            if *system_program.key != system_program::id() { return Err(IncorrectProgramId); }

            finish_deposit(&signer, program_id, program_account, &mut deposit_account, system_program)
        },
        InitWithdraw { recipient, amount, proof, public_inputs } => {
            // Program account
            let program_account = next_account_info(account_info_iter)?;
            let data = &mut program_account.data.borrow_mut()[..];
            let program_account = ProgramAccount::new(&program_account, data, program_id)?;

            // Withdraw account
            let withdraw_account = next_account_info(account_info_iter)?;
            let data = &mut withdraw_account.data.borrow_mut()[..];
            let mut withdraw_account = ProofVerificationAccount::new(&withdraw_account, data, program_id)?;

            // CUs consumed until this point: ~ 7270
            init_withdraw(&program_account, &mut withdraw_account, amount, recipient, &proof, public_inputs)
        },
        VerifyWithdraw => {
            // Withdraw account (~ 5600)
            let withdraw_account = next_account_info(account_info_iter)?;
            let data = &mut withdraw_account.data.borrow_mut()[..];
            let mut withdraw_account = ProofVerificationAccount::new(&withdraw_account, data, program_id)?;    

            verify_withdraw(&mut withdraw_account)
        },
        FinishWithdraw => {
            // Relayer (aka signer/payer)
            let relayer = next_account_info(account_info_iter)?;
            if !relayer.is_signer { return Err(SenderIsNotSigner.into()); }

            // Program account
            let program_account = next_account_info(account_info_iter)?;

            // Withdraw account
            let withdraw_account = next_account_info(account_info_iter)?;
            let data = &mut withdraw_account.data.borrow_mut()[..];
            let mut withdraw_account = ProofVerificationAccount::new(&withdraw_account, data, program_id)?;

            // Recipient
            let recipient = next_account_info(account_info_iter)?;

            finish_withdraw(program_id, program_account, &mut withdraw_account, relayer, recipient)
        }
        _ => Err(InvalidArgument)
    }
}

/// Starts the deposit and calculates the first hash iteration
fn init_deposit(
    program_account: &mut ProgramAccount,
    deposit_account: &mut DepositHashingAccount,
    amount: u64,
    commitment: ScalarLimbs
) -> ProgramResult {

    // Check amount
    if amount < MINIMUM_AMOUNT { return Err(InvalidAmount.into()); }

    // Check commitment
    program_account.can_insert_commitment(commitment)?;

    // Reset hashing values
    let leaf_index = program_account.leaf_pointer();
    deposit_account.set_committed_amount(amount);
    deposit_account.set_current_hash_iteration(poseidon::ITERATIONS as u16);
    deposit_account.set_current_hash_tree_position(0);
    deposit_account.set_opening(&merkle::opening(program_account.merkle_tree, leaf_index as usize))?;
    deposit_account.set_leaf_index(leaf_index);

    // Add commitment to hashing state and finished hash store
    let commitment = from_limbs_mont(&commitment);
    deposit_account.set_finished_hash(0, commitment);
    deposit_account.set_hashing_state([commitment, Scalar::zero(), Scalar::zero()]);

    // Start first hash
    compute_deposit(deposit_account)
}

/// Calculates the hash iterations
fn compute_deposit(
    deposit_account: &mut DepositHashingAccount
) -> ProgramResult {

    // Fetch values
    let mut tree_position = deposit_account.get_current_hash_tree_position();
    let mut iteration = deposit_account.get_current_hash_iteration();
    let mut state = deposit_account.get_hashing_state();

    // Move to next tree level
    if iteration as usize == poseidon::ITERATIONS {
        // Save hash
        let previous_hash = state[0];
        deposit_account.set_finished_hash(tree_position as usize, previous_hash);

        // Reset values
        let index = deposit_account.get_leaf_index() >> (tree_position as usize);
        let neighbour = deposit_account.get_neighbour(tree_position as usize);
        let last_hash_is_left = (index & 1) == 0;
        tree_position += 1;
        iteration = 0;

        // Set new inputs
        state[0] = Scalar::zero();
        state[1] = if last_hash_is_left { previous_hash } else { neighbour };
        state[2] = if last_hash_is_left { neighbour } else { previous_hash };

        // Finished
        if tree_position as usize == super::state::TREE_HEIGHT + 1 { return Ok(()) }
    }

    // Partial hashing
    let hash = Poseidon2::new().partial_hash(iteration as usize, state[0], state[1], state[2]);
    deposit_account.set_hashing_state(hash);

    // Save values
    iteration += 1;
    deposit_account.set_current_hash_iteration(iteration);
    deposit_account.set_current_hash_tree_position(tree_position);

    Ok(())
}

/// Save the hashes after finishing the deposit hashes
fn finalize_deposit(
    program_account: &mut ProgramAccount,
    deposit_account: &mut DepositHashingAccount,
) -> ProgramResult {
    let tree_position = deposit_account.get_current_hash_tree_position() as usize;
    let iteration = deposit_account.get_current_hash_iteration() as usize;

    // Assert that hashing is finished
    if iteration != poseidon::ITERATIONS || tree_position != super::state::TREE_HEIGHT {
        return Err(DidNotFinishHashing.into())
    }

    // Store last hash
    deposit_account.set_finished_hash(tree_position, deposit_account.get_hashing_state()[0]);

    // Store all hashes
    program_account.add_commitment(deposit_account.get_finished_hashes_storage())?;

    // Reset the hash process values
    deposit_account.set_current_hash_iteration(poseidon::ITERATIONS as u16);
    deposit_account.set_current_hash_tree_position(0);

    Ok(())
}

/// Runs the last hash iteration and stores the commitment and hash values
fn finish_deposit<'a>(
    sender: & AccountInfo<'a>,
    program_id: &Pubkey,
    program_account: & AccountInfo<'a>,
    deposit_account: &mut DepositHashingAccount,
    system_program: & AccountInfo<'a>,
) -> ProgramResult {
    let amount;

    {
        let data = &mut program_account.data.borrow_mut()[..];
        let mut program_account = ProgramAccount::new(&program_account, data, program_id)?;

        // Check if hashing is finished
        // Save the commitment and calculated values in the merkle tree
        finalize_deposit(&mut program_account, deposit_account)?;

        // Fetch the amount
        amount = deposit_account.get_committed_amount();
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
fn init_withdraw(
    program_account: &ProgramAccount,
    proof_account: &mut ProofVerificationAccount,
    amount: u64,
    recipient: Pubkey,
    proof: &[u8],
    public_inputs: [[u8; 32]; super::instruction::PUBLIC_INPUTS_COUNT],
) -> ProgramResult {
    let program_recipient = Pubkey::new_from_array(vec_to_array_32(proof_account.recipient.to_vec()));
    if program_recipient == recipient {
        return verify_withdraw(proof_account);
    }

    // Verify all public inputs
    verify_public_inputs(program_account, amount, public_inputs)?;

    // Init values (~ 33081 CUs)
    let proof = Proof::from_bytes(proof).unwrap();
    proof_account.init(amount, recipient, proof, public_inputs)?;

    // Start with computation
    verify_withdraw(proof_account)?;

    Ok(())
}

pub fn verify_public_inputs(
    program_account: &ProgramAccount,
    amount: u64,
    public_inputs: [[u8; 32]; super::instruction::PUBLIC_INPUTS_COUNT],
) -> ProgramResult {
    // Check the amount
    if amount < MINIMUM_AMOUNT { return Err(InvalidAmount.into()); }

    { // Check if nullifier does not already exist (~ 35000 CUs)
        let nullifier_hash = bytes_to_limbs(&public_inputs[1]);
        program_account.can_insert_nullifier_hash(nullifier_hash)?;
    }

    // Check merkle root
    let root = bytes_to_limbs(&public_inputs[0]);
    if !program_account.is_root_valid(root) { return Err(InvalidMerkleRoot.into()) }

    Ok(())
}

pub fn verify_withdraw(
    account: &mut ProofVerificationAccount,
) -> ProgramResult {
    let iteration = account.get_iteration();

    // Prepare inputs
    if iteration < PREPARE_INPUTS_ITERATIONS {
        if iteration == 0 { account.set_round(0); }
        partial_prepare_inputs(account, iteration)?;
    } else

    // Miller loop
    if iteration < PREPARE_INPUTS_ITERATIONS + MILLER_LOOP_ITERATIONS {
        let base = PREPARE_INPUTS_ITERATIONS;
        if iteration == base { account.set_round(0); }

        partial_miller_loop(account, iteration - base)?;
    } else

    // Final exponentiation
    if iteration < PREPARE_INPUTS_ITERATIONS + MILLER_LOOP_ITERATIONS + FINAL_EXPONENTIATION_ITERATIONS {
        let base = PREPARE_INPUTS_ITERATIONS + MILLER_LOOP_ITERATIONS;
        if iteration == base { account.set_round(0); }

        partial_final_exponentiation(account, iteration - base);
    } else {
        return Ok(())
    }

    account.set_iteration(iteration + 1);
    account.serialize();

    Ok(())
}

fn finish_withdraw(
    program_id: &Pubkey,
    program_account: &AccountInfo,
    proof_account: &mut ProofVerificationAccount,
    _relayer: &AccountInfo,
    recipient: &AccountInfo,
) -> ProgramResult {
    let iteration = proof_account.get_iteration();

    // Continue computation if not already finished
    if iteration < super::groth16::ITERATIONS {
        return verify_withdraw(proof_account);
    }

    // Check if proof is verified
    if !verify_proof(proof_account, iteration) {
        return Err(InvalidProof.into())
    }

    // Save nullifier hash
    {
        let nullifier_hash = &proof_account.inputs_be[..32];
        let nullifier_hash: Vec<u8> = nullifier_hash.iter().copied().rev().collect();
        let nullifier_hash = bytes_to_limbs(&nullifier_hash);

        let data = &mut program_account.data.borrow_mut()[..];
        let mut program_account = ProgramAccount::new(&program_account, data, program_id)?;
        program_account.insert_nullifier_hash(nullifier_hash)?;
    }

    // Pay the signer (in most cases the relayer) RELAYER_FEE
    //TODO: Send relayer his funds

    // Check if recipient is correct
    let program_recipient = Pubkey::new_from_array(vec_to_array_32(proof_account.recipient.to_vec()));
    if program_recipient != *recipient.key {
        return Err(InvalidRecipient.into())
    }

    // Transfer funds using owned bank account
    let amount = u64::from_le_bytes([
        proof_account.amount[0], proof_account.amount[1],
        proof_account.amount[2], proof_account.amount[3],
        proof_account.amount[4], proof_account.amount[5],
        proof_account.amount[6], proof_account.amount[7],
    ]);
    **program_account.try_borrow_mut_lamports()? -= amount;
    **recipient.try_borrow_mut_lamports()? += amount;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::merkle::node;
    use super::super::state::*;
    use super::super::poseidon::DepositHashingAccount;

    fn send_deposit(program_account: &mut ProgramAccount, deposit_account: &mut DepositHashingAccount, commitment: &str) {
        let commitment = bytes_to_limbs(&to_bytes_le_mont(from_str_10(commitment)));
        init_deposit(program_account, deposit_account, LAMPORTS_PER_SOL, commitment).unwrap();

        // Deposit Computation
        for _ in 0..poseidon::ITERATIONS * (TREE_HEIGHT) - 1 {
            compute_deposit(deposit_account).unwrap();
        }
        
        // Finish Deposit
        finalize_deposit(program_account, deposit_account).unwrap();
    }

    fn test_compute_merkle_tree(commitment: &str, hashes: [(usize, &str); TREE_HEIGHT + 1]) {
        // Init Storage
        let mut data = [0 as u8; ProgramAccount::TOTAL_SIZE];
        let mut program_account = ProgramAccount::from_data(&mut data).unwrap();
        let mut data = [0 as u8; DepositHashingAccount::TOTAL_SIZE];
        let mut deposit_account = DepositHashingAccount::from_data(&mut data).unwrap();

        send_deposit(&mut program_account, &mut deposit_account, commitment);

        // Check hashes
        for (i, (index, str)) in hashes.iter().enumerate() {
            println!("{}", i);
            assert_eq!(
                from_str_10(str),
                node(&program_account.merkle_tree, TREE_HEIGHT - i, *index)
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
        let mut data = [0 as u8; ProgramAccount::TOTAL_SIZE];
        let mut program_account = ProgramAccount::from_data(&mut data).unwrap();
        let mut data = [0 as u8; DepositHashingAccount::TOTAL_SIZE];
        let mut deposit_account = DepositHashingAccount::from_data(&mut data).unwrap();

        // Deposit 0
        send_deposit(&mut program_account, &mut deposit_account, "13742746012751083277892228377764260000239534456878525049335647276801809645457");
        assert_eq!(
            node(&program_account.merkle_tree, 0, 0),
            from_str_10("12986367982040817160540332433371902400206274668282942567679375016030163683535")
        );

        // Deposit 1
        send_deposit(&mut program_account, &mut deposit_account, "17598496762772913768842234443529375820067611385012556852766388745086067053344");

        // Explicit first hash check
        assert_eq!(
            node(&program_account.merkle_tree, TREE_HEIGHT - 1, 0),
            from_str_10("9250582908633439312254427073747285879616368085421326049753099391976074597705")
        );

        // Check hashes
        let poseidon = Poseidon2::new();
        let mut hash = poseidon.full_hash(
            from_str_10("13742746012751083277892228377764260000239534456878525049335647276801809645457"),
            from_str_10("17598496762772913768842234443529375820067611385012556852766388745086067053344")
        );
        for i in 1..=TREE_HEIGHT {
            println!("{}", i);
            assert_eq!(hash, node(&program_account.merkle_tree, TREE_HEIGHT - i, 0));
            hash = poseidon.full_hash(hash, Scalar::zero());
        }

        // Explicit root check
        assert_eq!(
            node(&program_account.merkle_tree, 0, 0),
            from_str_10("14179231500255854581437255655788433381559803720571568228288219562259757099116")
        );
    }

    #[test]
    fn test_compute_roots() {
        let mut data = [0 as u8; ProgramAccount::TOTAL_SIZE];
        let mut program_account = ProgramAccount::from_data(&mut data).unwrap();
        let mut data = [0 as u8; DepositHashingAccount::TOTAL_SIZE];
        let mut deposit_account = DepositHashingAccount::from_data(&mut data).unwrap();

        send_deposit(&mut program_account, &mut deposit_account, "2691871084338929956037274350088764461609286924004272324652786264956258392689");
        send_deposit(&mut program_account, &mut deposit_account, "7894767338664390818553781660535492406045127772328385874526611296339530133956");
        send_deposit(&mut program_account, &mut deposit_account, "7368144767547615303698512650401844721079039558002839879495553168698049012372");

        let hash0 = poseidon::Poseidon2::new().full_hash(
            from_str_10("2691871084338929956037274350088764461609286924004272324652786264956258392689"),
            from_str_10("7894767338664390818553781660535492406045127772328385874526611296339530133956")
        );
        let hash1 = poseidon::Poseidon2::new().full_hash(
            from_str_10("7368144767547615303698512650401844721079039558002839879495553168698049012372"),
            Scalar::zero()
        );
        let hash2 = poseidon::Poseidon2::new().full_hash(hash0, hash1);

        assert_eq!(hash1, from_str_10("16760737614838584501323442907641218354264916733890034990263802893914537956977"));
        assert_eq!(hash2, from_str_10("19694870396733588453229939445490507461522064973149372099027071294247270209313"));

        assert_eq!(node(&program_account.merkle_tree, TREE_HEIGHT - 1, 0), hash0);
        assert_eq!(node(&program_account.merkle_tree, TREE_HEIGHT - 1, 1), hash1);
        assert_eq!(node(&program_account.merkle_tree, TREE_HEIGHT - 2, 0), hash2);

        assert_eq!(
            node(&program_account.merkle_tree, 0, 0),
            from_str_10("4397724660288410284880274442375722705377633912702097379882757919297316383721")
        );
    }
}