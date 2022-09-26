use borsh::{BorshDeserialize, BorshSerialize};
use elusiv_types::{accounts::PDAAccountData, BorshSerDeSized, SizedAccount};
use elusiv_utils::guard;
use solana_program::{entrypoint::ProgramResult, pubkey::Pubkey};
use crate::macros::{elusiv_account, BorshSerDeSized};
use crate::error::ElusivWardenNetworkError::WardenRegistrationError;
use crate::{apa::APAECert, warden::ElusivWardenID};

pub trait WardenNetwork {
    const TYPE: WardenNetworkType;
    const SIZE: WardenNetworkSize;
}

pub enum WardenNetworkType {
    Basic,
    Full,
    Mixed,
}

pub enum WardenNetworkSize {
    Fixed(usize),
    Dynamic(usize, usize),
}

impl WardenNetworkSize {
    pub const fn members_count(&self) -> usize {
        match self {
            WardenNetworkSize::Fixed(m) => *m,
            WardenNetworkSize::Dynamic(_, m) => *m,
        }
    }
}

macro_rules! warden_network {
    ($ty: ident, $account_ty: ident, $seed: expr, $type: expr, $size: expr, $members_count: expr) => {
        pub struct $ty {}

        impl WardenNetwork for $ty {
            const TYPE: WardenNetworkType = $type;
            const SIZE: WardenNetworkSize = $size;
        }

        #[elusiv_account(pda_seed = $seed)]
        pub struct $account_ty {
            pda_data: PDAAccountData,

            members: [u32; $ty::SIZE.members_count()],
            members_count: u32,
        }

        impl<'a> $account_ty<'a> {
            pub fn copy_members(&self) -> Vec<u8> {
                self.members.to_vec()
            }
        }
    };
}

pub const BASIC_WARDEN_GENESIS_NETWORK_SIZE_LIMIT: usize = 1024;

warden_network! {
    ElusivBasicWardenNetwork,
    ElusivBasicWardenNetworkAccount,
    b"basic_wardens",
    WardenNetworkType::Mixed,
    WardenNetworkSize::Dynamic(0, BASIC_WARDEN_GENESIS_NETWORK_SIZE_LIMIT),
    BASIC_WARDEN_GENESIS_NETWORK_SIZE_LIMIT
}

pub const FULL_WARDEN_GENESIS_NETWORK_SIZE: usize = 6;

warden_network! {
    ElusivFullWardenNetwork,
    ElusivFullWardenNetworkAccount,
    b"full_wardens",
    WardenNetworkType::Full,
    WardenNetworkSize::Fixed(FULL_WARDEN_GENESIS_NETWORK_SIZE),
    FULL_WARDEN_GENESIS_NETWORK_SIZE
}

#[elusiv_account(pda_seed = b"full_wardens_genesis")]
pub struct FullWardenRegistrationAccount {
    pda_data: PDAAccountData,

    wardens: [Pubkey; FULL_WARDEN_GENESIS_NETWORK_SIZE],
    applications: [FullWardenRegistrationApplication; FULL_WARDEN_GENESIS_NETWORK_SIZE],
    applications_count: u32,
    confirmations: [bool; FULL_WARDEN_GENESIS_NETWORK_SIZE],
}

impl<'a> FullWardenRegistrationAccount<'a> {
    fn application_finished(applicants_count: u32) -> bool {
        applicants_count == FULL_WARDEN_GENESIS_NETWORK_SIZE as u32
    }

    pub fn register_applicant(
        &mut self,
        warden: &Pubkey,
        warden_id: ElusivWardenID,
        application: FullWardenRegistrationApplication,
    ) -> ProgramResult {
        let applicants_count = self.get_applications_count();

        guard!(!Self::application_finished(applicants_count), WardenRegistrationError);
        guard!(warden_id == applicants_count, WardenRegistrationError);

        for i in 0..applicants_count as usize {
            guard!(*warden != self.get_wardens(i), WardenRegistrationError);
        }

        self.set_wardens(applicants_count as usize, warden);
        self.set_applications(applicants_count as usize, &application);
        self.set_applications_count(&(applicants_count + 1));

        Ok(())
    }

    pub fn confirm_all_other_applications(
        &mut self,
        warden: &Pubkey,
        warden_id: ElusivWardenID,
    ) -> ProgramResult {
        let applicants_count = self.get_applications_count();

        guard!(Self::application_finished(applicants_count), WardenRegistrationError);
        guard!(self.get_wardens(warden_id as usize) == *warden, WardenRegistrationError);

        self.set_confirmations(warden_id as usize, &true);

        Ok(())
    }
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized)]
pub struct FullWardenRegistrationApplication {
    pub apae_cert: APAECert,
}

#[elusiv_account(pda_seed = b"basic_warden_application")]
pub struct BasicWardenApplicationAccount {
    pda_data: PDAAccountData,
    warden: Pubkey,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_full_warden_network_size_overflow() {
        assert!(FULL_WARDEN_GENESIS_NETWORK_SIZE <= (u32::MAX as usize));
    }
}