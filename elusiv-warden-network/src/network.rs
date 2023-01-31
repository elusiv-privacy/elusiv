use crate::warden::ElusivWardenID;
use crate::{error::ElusivWardenNetworkError, warden::BasicWardenFeatures};
use elusiv_proc_macros::elusiv_account;
use elusiv_types::{PDAAccountData, TOKENS};
use elusiv_utils::guard;
use solana_program::entrypoint::ProgramResult;

pub trait WardenNetwork {
    const SIZE: NetworkSize;
}

pub enum NetworkSize {
    Fixed(usize),
    Dynamic(usize, usize),
}

impl NetworkSize {
    pub const fn max(&self) -> usize {
        match self {
            NetworkSize::Fixed(m) => *m,
            NetworkSize::Dynamic(_, m) => *m,
        }
    }
}

pub struct ElusivBasicWardenNetwork;

impl WardenNetwork for ElusivBasicWardenNetwork {
    const SIZE: NetworkSize = NetworkSize::Dynamic(0, 512);
}

#[elusiv_account]
pub struct BasicWardenNetworkAccount {
    pda_data: PDAAccountData,

    members_count: u32,
    members: [ElusivWardenID; ElusivBasicWardenNetwork::SIZE.max()],
    features: [BasicWardenFeatures; ElusivBasicWardenNetwork::SIZE.max()],
    tokens: [[bool; TOKENS.len()]; ElusivBasicWardenNetwork::SIZE.max()],
}

impl<'a> BasicWardenNetworkAccount<'a> {
    pub fn try_add_member(
        &mut self,
        warden_id: ElusivWardenID,
        features: &BasicWardenFeatures,
        supported_tokens: &[bool; TOKENS.len()],
    ) -> ProgramResult {
        let members_count = self.get_members_count();
        guard!(
            (members_count as usize) < ElusivBasicWardenNetwork::SIZE.max(),
            ElusivWardenNetworkError::WardenRegistrationError
        );

        self.set_members(members_count as usize, &warden_id);
        self.set_features(members_count as usize, features);
        self.set_tokens(members_count as usize, supported_tokens);
        self.set_members_count(&(members_count + 1));

        Ok(())
    }
}
