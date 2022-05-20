use solana_program::{entrypoint::ProgramResult, account_info::AccountInfo};
use crate::macros::guard;
use crate::state::queue::{
    RingQueue,
    SendProofQueue,SendProofQueueAccount,
    MergeProofQueue,MergeProofQueueAccount,
    MigrateProofQueue,MigrateProofQueueAccount,
};
use crate::proof::{VerificationAccount, MAX_VERIFICATION_ACCOUNTS_COUNT};
use crate::error::ElusivError::{InvalidAccount, ComputationIsNotYetFinished};

/// Dequeues a proof request and places it into a `VerificationAccount`
macro_rules! init_proof {
    ($fn_name: ident, $queue_ty: ty, $queue_account_ty: ty) => {
        pub fn $fn_name<'a>(
            queue: &mut $queue_account_ty,
            verification_account: &mut VerificationAccount,
        
            verification_account_index: u64,
        ) -> ProgramResult {
            guard!(verification_account_index < MAX_VERIFICATION_ACCOUNTS_COUNT, InvalidAccount);
            guard!(!verification_account.get_is_active(), ComputationIsNotYetFinished);
        
            let mut queue = <$queue_ty>::new(queue);
            let request = queue.dequeue_first()?;
        
            Ok(())
        }
    };
}

init_proof!(init_send_proof, SendProofQueue, SendProofQueueAccount);
init_proof!(init_merge_proof, MergeProofQueue, MergeProofQueueAccount);
init_proof!(init_migrate_proof, MigrateProofQueue, MigrateProofQueueAccount);