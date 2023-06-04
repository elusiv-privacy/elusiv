use super::{commitment::COMMITMENT_QUEUE_LEN, queue::queue_account};
use crate::commitment::MT_HEIGHT;
use elusiv_proc_macros::elusiv_account;
use elusiv_types::{
    accounts::PDAAccountData, BorshSerDeSized, ChildAccount, ElusivOption, ParentAccount,
};
use elusiv_utils::two_pow;
use solana_program::{entrypoint::ProgramResult, pubkey::Pubkey};

pub type CommitmentMetadata = [u8; 17];

queue_account!(
    MetadataQueue,
    MetadataQueueAccount,
    COMMITMENT_QUEUE_LEN,
    CommitmentMetadata,
);

const VALUES_PER_METADATA_CHILD_ACCOUNT: usize = two_pow!(16);
const ACCOUNTS_COUNT: usize = two_pow!(MT_HEIGHT as u32) / VALUES_PER_METADATA_CHILD_ACCOUNT;

#[cfg(test)]
const_assert_eq!(ACCOUNTS_COUNT, 16);

pub struct MetadataChildAccount;

impl ChildAccount for MetadataChildAccount {
    const INNER_SIZE: usize = VALUES_PER_METADATA_CHILD_ACCOUNT * CommitmentMetadata::SIZE;
}

#[elusiv_account(parent_account: { child_account_count: ACCOUNTS_COUNT, child_account: MetadataChildAccount }, eager_type: true)]
pub struct MetadataAccount {
    #[no_getter]
    #[no_setter]
    pda_data: PDAAccountData,
    pubkeys: [ElusivOption<Pubkey>; ACCOUNTS_COUNT],

    pub next_metadata_ptr: u32,
}

impl<'a, 'b, 't> MetadataAccount<'a, 'b, 't> {
    pub fn add_commitment_metadata(&mut self, metadata: &CommitmentMetadata) -> ProgramResult {
        let metadata_index = self.get_next_metadata_ptr() as usize;
        let (child_index, index) = Self::child_account_and_local_index(metadata_index);

        self.execute_on_child_account_mut(child_index, |data| {
            let offset = index * CommitmentMetadata::SIZE;
            let slice = &mut data[offset..offset + CommitmentMetadata::SIZE];
            slice.copy_from_slice(&metadata[..]);
        })?;

        self.set_next_metadata_ptr(&(metadata_index as u32 + 1));

        Ok(())
    }

    #[cfg(feature = "elusiv-client")]
    pub fn get_commitment_metadata(
        &self,
        index: usize,
    ) -> Result<CommitmentMetadata, solana_program::program_error::ProgramError> {
        use crate::error::ElusivError;

        let metadata_index = self.get_next_metadata_ptr() as usize;
        crate::macros::guard!(index < metadata_index, ElusivError::MissingValue);

        let (child_index, index) = Self::child_account_and_local_index(index);

        self.execute_on_child_account(child_index, |data| {
            let offset = index * CommitmentMetadata::SIZE;
            data[offset..offset + CommitmentMetadata::SIZE]
                .try_into()
                .unwrap()
        })
    }

    fn child_account_and_local_index(metadata_index: usize) -> (usize, usize) {
        let child_index = metadata_index / VALUES_PER_METADATA_CHILD_ACCOUNT;
        let index = metadata_index % VALUES_PER_METADATA_CHILD_ACCOUNT;

        (child_index, index)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::macros::parent_account;

    fn usize_to_metadata(u: usize) -> CommitmentMetadata {
        let mut metadata = [0; CommitmentMetadata::SIZE];
        metadata[..8].copy_from_slice(&(u as u64).to_le_bytes());
        metadata
    }

    #[test]
    fn test_add_commitment_metadata() {
        parent_account!(mut metadata_account, MetadataAccount);

        assert_ne!(usize_to_metadata(0), usize_to_metadata(1));

        for i in 0..MT_HEIGHT {
            metadata_account
                .add_commitment_metadata(&usize_to_metadata(i))
                .unwrap();
        }

        for i in 0..MT_HEIGHT {
            assert_eq!(
                metadata_account.get_commitment_metadata(i).unwrap(),
                usize_to_metadata(i)
            );
        }
    }
}
