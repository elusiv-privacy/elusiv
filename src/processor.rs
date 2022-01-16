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
    log::sol_log_compute_units,
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

        sol_log_compute_units();

        // 0. [signer] Signer
        let sender = next_account_info(account_info_iter)?;

        // 1. [owned, writable] Program main account
        // TODO: Add program id verification
        let program_account = next_account_info(account_info_iter)?;
        if program_account.owner != program_id { return Err(InvalidStorageAccount.into()); }
        let data = &mut program_account.data.borrow_mut()[..];
        let mut storage = StorageAccount::from(data)?;

        match instruction {
            InitDeposit { amount, commitment } =>  {
                if !sender.is_signer { return Err(SenderIsNotSigner.into()); }

                Self::init_deposit(&mut storage, amount, commitment)
            },
            ComputeDeposit => {
                Self::compute_deposit(&mut storage)
            },
            FinishDeposit => {
                if !sender.is_signer { return Err(SenderIsNotSigner.into()); }

                // 2. [] System program
                let system_program = next_account_info(account_info_iter)?;
                if *system_program.key != system_program::id() { return Err(IncorrectProgramId); }

                Self::finish_deposit(program_id, program_account, sender, system_program)
            },
            Withdraw { amount, proof, nullifier_hash, root } => {
                if !sender.is_signer { return Err(SenderIsNotSigner.into()); }

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
        sol_log_compute_units();
        Self::compute_deposit(storage)
    }

    /// Calculates the hash iterations
    fn compute_deposit(storage: &mut StorageAccount) -> ProgramResult {
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
        sol_log_compute_units();
        let hash = Poseidon2::new().partial_hash(current_iteration as usize, state[0], state[1], state[2]);
        storage.set_hashing_state(hash);
        sol_log_compute_units();

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

        sol_log_compute_units();
        {
            let data = &mut program_account.data.borrow_mut()[..];
            let mut storage = StorageAccount::from(data)?;

            // Compute last hash iteration
            //Self::compute_deposit(&mut storage)?;
            sol_log_compute_units();

            // Check if hashing is finished
            if storage.get_current_hash_iteration() != 0 || (storage.get_current_hash_tree_position() as usize) <= super::state::TREE_HEIGHT {
                return Err(DidNotFinishHashing.into())
            }
            sol_log_compute_units();

            // Fetch the amount
            amount = storage.get_committed_amount();
            sol_log_compute_units();

            // Save the commitment and calculated values in the merkle tree
            storage.add_commitment()?;
            sol_log_compute_units();
        }

        sol_log_compute_units();
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
        from_str_16,
        bytes_to_limbs,
        to_bytes_le,
    };
    use super::super::merkle::{
        get_node,
        insert_hashes,
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

    fn test_compute_merkle_tree(commitment: &str, index: usize, hashes: [(usize, &str); TREE_HEIGHT + 1]) {
        // Init Storage
        let mut data = init_merkle_tree(index);
        let mut storage = StorageAccount::from(&mut data).unwrap();

        // Init Deposit
        let commitment = bytes_to_limbs(&to_bytes_le(from_str_16(commitment).unwrap()));
        Processor::init_deposit(&mut storage, LAMPORTS_PER_SOL, commitment).unwrap();

        // Deposit Computation
        for _ in 0..poseidon::ITERATIONS * (TREE_HEIGHT + 1) - 2 {
            Processor::compute_deposit(&mut storage).unwrap();
        }
        
        // Finish Deposit
        finish_deposit_mock(&mut storage);

        // Check hashes
        for (i, (index, str)) in hashes.iter().enumerate() {
            assert_eq!(
                from_str_16(str).unwrap(),
                get_node(&storage.merkle_tree, TREE_HEIGHT - i, *index)
            );
        }
    }

    fn finish_deposit_mock(storage: &mut StorageAccount) {
        Processor::compute_deposit(storage).unwrap();

        let hashes = storage.get_finished_hashes_storage();
        let leaf_index = storage.leaf_pointer() as usize;
        insert_hashes(&mut storage.merkle_tree, hashes, leaf_index);
    }

    #[test]
    fn test_full_merkle_computation() {
        test_compute_merkle_tree(
            "0x276A0A462A3D78551D2C5328E5723F062079EA04CEE957E97CE2A87D3559E5C4",
            3,
            [
                (3, "0x276A0A462A3D78551D2C5328E5723F062079EA04CEE957E97CE2A87D3559E5C4"),
                (1, "0x084FF51394066A9AC8AA73D2C82967079E448B993248B406EAD818E94E9D141C"),
                (0, "0x08469DAFFB92934E35F017B41038B4DF6B14C4D035DD6808BE97E487C9A693B4"),
                (0, "0x072176F247D1B74056D9CB0CD18E3A0B6538D95DADF47BE6F47B3606497C3505"),
                (0, "0x0CB2B5FE10C7DFE6EA12BD77DE120B8F4BFE8965991FC8FF6FD8041D938F468D"),
                (0, "0x0658BB65CD7775A0AD02C0E381146FF6B134F1D491E50893802EA833024CF95C"),
                (0, "0x10E473EE0DF4C9567D74C77A29B10A5139A96466866274596B1631A2CC413A7B"),
                (0, "0x2A364872AB8200187AB38E41A2DD38ECB3E006637AADAB6942EF44B7E6746631"),
                (0, "0x0456562E21FBE9CFEF7A424466EE1D97130A04A8031E601796FA0B9A7646742F"),
                (0, "0x1927261B8B0AD311B3D5DAD8BFD44FEC6641FB7A662DE899B0EE679CC67B28ED"),
                (0, "0x0497F5187616DCD4EA0F3045C3C9D220A4F9E6BC3B77FE168012403CFA867411"),
                (0, "0x2953F0C53377A2D78A6FC3F998D6B7F773061C56E2BD10F49EC8CA049F34672D"),
                (0, "0x2DE70D1BD929B29AE81747B88B12CAE4A1811FB3FA564BCF137C4AA86CB97911"),
            ]
        );

        test_compute_merkle_tree(
            "0x00AED8EB45CE696AE18A312CA7EF19EA0498794331D9A4262173E9970E06A488",
            21,
            [
                (21, "0x00AED8EB45CE696AE18A312CA7EF19EA0498794331D9A4262173E9970E06A488"),
                (10, "0x092CF079C53FE9FD62BF24F63937E7CB7066342ACD2EEF31E517514685C858C8"),
                (5, "0x1D16A55D6726721ECE8AAED1160BC97AB8C54C6174FA858BC41795C20843DC95"),
                (2, "0x1776B3164EF6C5E2C7EE5A72C438718D2D1CFB63D87BC8BFE1764CD4BEB6CA31"),
                (1, "0x12068A6D69912EBED55E782FA9EEB0162242198DB85ABE6CBB97ABE057940078"),
                (0, "0x2B2EBE8EBBE10224C5037D8F40A46E1ED616B424C0BAD4FA808635E5696CB6DC"),
                (0, "0x28D23095F682CE8ADB779CC90F2F86727332217E296BAB1B3EB588552FE2DF44"),
                (0, "0x07C53408D753982F985BD9D443062501ADF5F431B68C4ED2CC0F666C54ADACF8"),
                (0, "0x2C112A620E957332956611A6CD573B1CAD51A7B531F2B796E7D19082C87F81A5"),
                (0, "0x17882EB07E6A18070B5AB3D719994C68FFD33A894BD336CD2935B90BB4509701"),
                (0, "0x17C258250F0E5164BD62907BFC83445051804C998D2E4FC18CC7A6624F320DF8"),
                (0, "0x1B301FA32BD45F8F4ADEDFBCA570E403970553D52ED2E3FD8AF217F07A85363D"),
                (0, "0x1AFCF43FF94DC437F1F0D656AE89AC7755E0A4E19D08A4BDD107E8BD1413730D"),
            ]
        );

        test_compute_merkle_tree(
            "0x1F206986101D08702D563E37FD60D7687ED5BD52B35788B62C7948BF37715A44",
            0,
            [
                (0, "0x1F206986101D08702D563E37FD60D7687ED5BD52B35788B62C7948BF37715A44"),
                (0, "0x1EA0040FA4A1ECACE2627041E1B0691E0E671C8936575D7E5A62A473AB1C93A2"),
                (0, "0x0AB92F3F5AE32E19A12E545E4FF97D05C77C021D0710740F85907546731034B4"),
                (0, "0x16F0716422D27692282FFAC863BF630E933C15685FB814E8793B8CB0E13D57AB"),
                (0, "0x2A1FB9024E9CC3C4FF18574ACBA59435B9D72CBF8AE51C682AF1AE2D423E34C8"),
                (0, "0x1E9339A4431F07B965D13E44CD10CF779450219E0CAC8CEC009C698B668DDB11"),
                (0, "0x1A1E6323CF14E130CB68B4B4DC5EAFECC73FD726F2DF5A571F71780ED774A2C1"),
                (0, "0x2C346934449E9C8F82709907E5472F06014353D203D711AD7E608F7723777558"),
                (0, "0x13696A21B68C2386813C30A839830CA0EAC776610D3F44FE2745C0629C531476"),
                (0, "0x2E0BB596EE4648E776B9EDAE496DB0A5579E4DBCEC64F5164152D9A5089F0D49"),
                (0, "0x087F2819E07163A0F451331D624AD5E6947D9956D54CADF31252BEF4E0B68D8A"),
                (0, "0x1762B520B2B3A6D61076EB63AC04E5F2C1DEACC12101ED91578FAA59FC594F4F"),
                (0, "0x2FA2FECA0F315B61C50BC96857AE4562EFD1DAC3B94D48EBDFFAEDECD020CC24"),
            ]
        );

        test_compute_merkle_tree(
            "0x121C2AF32EBBAB8932DFCBC77B3A942F5A4E1040EE7157C291131B002F387C00",
            341,
            [
                (341, "0x121C2AF32EBBAB8932DFCBC77B3A942F5A4E1040EE7157C291131B002F387C00"),
                (170, "0x010968E7FDB109D2305DDFEB1ACE34DCA79A88D300D9C25A10A7D2537EAF62AA"),
                (85, "0x148F2C414B470D95E23649FAF65EC2A3B37324BA041248A15A0B063F400CA61F"),
                (42, "0x1EB8F8F4AAF15E9A6F6F1984353BE185FC45C41D732FBF649FFE78D5B9E53FD0"),
                (21, "0x0504E361ABC101E9959BAC10B354F190C45A1CFD82D436A6723B9DFFC3B5851E"),
                (10, "0x16499C4041CFC662F4F9C71BE69D87FFFA6F7513FD5D39F3865770098485939D"),
                (5, "0x0B94E7CC5168C80A08C25ADDB4AE4D302456B405877BFB11F1AC8958397FB461"),
                (2, "0x1400EEA38ACB5C5C7DD0AB29A8C32F1CF84218904DC5E7D291276853D1887ED7"),
                (1, "0x049F4CCD3E43200E1BAFECA4CBCCD824027D773EC103A9D1F110AE051FDF8541"),
                (0, "0x295CBF7DFDFDED95E57A900C91DB01DCF180EB12871F60E3F5A621BDA10C2D9A"),
                (0, "0x051BA348F2812E2788E1B8E3D64F8BEAA24D4AA6314ED5BD30E9D22E1A2E4833"),
                (0, "0x2BDCEDBA3E35261D0F81F889594D7D00784865111D00C67EAD18033F26EB663A"),
                (0, "0x0BEBA834603258CABE341CCB98453AC8A3FA28C63ED3C153B9E909FD74C8BCEA"),
            ]
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
        let commitment = bytes_to_limbs(&to_bytes_le(from_str_16("0x121C2AF32EBBAB8932DFCBC77B3A942F5A4E1040EE7157C291131B002F387C00").unwrap()));
        Processor::init_deposit(&mut storage, LAMPORTS_PER_SOL, commitment).unwrap();

        // Deposit Computation
        for _ in 0..poseidon::ITERATIONS * (TREE_HEIGHT + 1) - 2 {
            Processor::compute_deposit(&mut storage).unwrap();
        }

        assert_eq!(original_merkle, clone_merkle(storage.merkle_tree));
    }
}