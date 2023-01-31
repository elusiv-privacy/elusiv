use crate::error::ElusivWardenNetworkError;
use crate::warden::ElusivWardenID;
use elusiv_proc_macros::elusiv_account;
use elusiv_types::{PDAAccountData, SPL_TOKEN_COUNT};
use elusiv_utils::guard;
use solana_program::entrypoint::ProgramResult;

pub trait WardenNetwork {
    const SIZE: WardenNetworkSize;
}

pub enum WardenNetworkSize {
    Fixed(usize),
    Dynamic(usize, usize),
}

impl WardenNetworkSize {
    pub const fn max(&self) -> usize {
        match self {
            WardenNetworkSize::Fixed(m) => *m,
            WardenNetworkSize::Dynamic(_, m) => *m,
        }
    }
}

pub struct ElusivBasicWardenNetwork;

impl WardenNetwork for ElusivBasicWardenNetwork {
    const SIZE: WardenNetworkSize = WardenNetworkSize::Dynamic(0, 512);
}

#[elusiv_account(eager_type: true)]
pub struct BasicWardenNetworkAccount {
    pda_data: PDAAccountData,

    members_count: u32,
    members: [ElusivWardenID; ElusivBasicWardenNetwork::SIZE.max()],
    tokens: [[bool; SPL_TOKEN_COUNT]; ElusivBasicWardenNetwork::SIZE.max()],
}

impl<'a> BasicWardenNetworkAccount<'a> {
    pub fn try_add_member(&mut self, warden_id: ElusivWardenID) -> ProgramResult {
        let members_count = self.get_members_count();
        guard!(
            (members_count as usize) < ElusivBasicWardenNetwork::SIZE.max(),
            ElusivWardenNetworkError::WardenRegistrationError
        );

        self.set_members(members_count as usize, &warden_id);
        self.set_members_count(&(members_count + 1));

        Ok(())
    }
}
