use crate::warden::{ElusivWardenID, Quote, WardenRegion};
use crate::{error::ElusivWardenNetworkError, warden::BasicWardenFeatures};
use elusiv_proc_macros::elusiv_account;
use elusiv_types::{ElusivOption, PDAAccountData, TOKENS};
use elusiv_utils::guard;
use solana_program::entrypoint::ProgramResult;
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;

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

macro_rules! warden_network {
    ($ident: ident, $size: expr) => {
        pub struct $ident;

        impl WardenNetwork for $ident {
            const SIZE: NetworkSize = $size;
        }
    };
}

warden_network!(ElusivBasicWardenNetwork, NetworkSize::Dynamic(0, 512));

#[elusiv_account]
pub struct BasicWardenNetworkAccount {
    pda_data: PDAAccountData,

    members_count: u32,
    members: [ElusivWardenID; ElusivBasicWardenNetwork::SIZE.max()],
    features: [BasicWardenFeatures; ElusivBasicWardenNetwork::SIZE.max()],
    tokens: [[bool; TOKENS.len()]; ElusivBasicWardenNetwork::SIZE.max()],
    region: [WardenRegion; ElusivApaWardenNetwork::SIZE.max()],
}

impl<'a> BasicWardenNetworkAccount<'a> {
    pub fn try_add_member(
        &mut self,
        warden_id: ElusivWardenID,
        features: &BasicWardenFeatures,
        region: &WardenRegion,
        supported_tokens: &[bool; TOKENS.len()],
    ) -> ProgramResult {
        let members_count = self.get_members_count();
        guard!(
            (members_count as usize) < ElusivBasicWardenNetwork::SIZE.max(),
            ElusivWardenNetworkError::WardenRegistrationError
        );

        self.set_members(members_count as usize, &warden_id);
        self.set_features(members_count as usize, features);
        self.set_region(members_count as usize, region);
        self.set_tokens(members_count as usize, supported_tokens);
        self.set_members_count(&(members_count + 1));

        Ok(())
    }

    pub fn update_region(
        &mut self,
        warden_id: ElusivWardenID,
        member_index: usize,
        region: &WardenRegion,
    ) -> ProgramResult {
        guard!(
            self.get_members(member_index) == warden_id,
            ElusivWardenNetworkError::InvalidInstructionData
        );

        self.set_region(member_index, region);

        Ok(())
    }
}

warden_network!(ElusivApaWardenNetwork, NetworkSize::Fixed(6));

#[elusiv_account]
pub struct ApaWardenNetworkAccount {
    pda_data: PDAAccountData,

    members_count: u32,
    apa_key: ElusivOption<Pubkey>,

    members: [ElusivWardenID; ElusivApaWardenNetwork::SIZE.max()],
    quotes: [Quote; ElusivApaWardenNetwork::SIZE.max()],
    exchange_keys: [Pubkey; ElusivApaWardenNetwork::SIZE.max()],
    confirmations: [bool; ElusivApaWardenNetwork::SIZE.max()],
}

impl<'a> ApaWardenNetworkAccount<'a> {
    pub fn is_application_phase(&self) -> bool {
        self.get_members_count() as usize == ElusivApaWardenNetwork::SIZE.max()
    }

    pub fn apply(&mut self, warden_id: ElusivWardenID, quote: &Quote) -> Result<u32, ProgramError> {
        guard!(
            self.is_application_phase(),
            ElusivWardenNetworkError::WardenRegistrationError
        );

        let members_count = self.get_members_count();
        self.set_members_count(&(members_count + 1));

        self.set_members(members_count as usize, &warden_id);
        self.set_quotes(members_count as usize, quote);
        self.set_exchange_keys(
            members_count as usize,
            &Pubkey::new_from_array(quote.user_data_bytes()),
        );

        Ok(members_count)
    }

    pub fn confirmation_message(&self) -> [u8; 32] {
        let hash = solana_program::hash::hash(self.quotes);
        hash.to_bytes()
    }

    pub fn confirm_others(
        &mut self,
        member_index: usize,
        signer: &Pubkey,
        confirmation_message: &[u8],
    ) -> ProgramResult {
        guard!(
            !self.is_application_phase(),
            ElusivWardenNetworkError::WardenRegistrationError
        );
        guard!(
            self.get_exchange_keys(member_index) == *signer,
            ElusivWardenNetworkError::InvalidSigner
        );
        guard!(
            self.confirmation_message() == confirmation_message,
            ElusivWardenNetworkError::InvalidSignature
        );

        self.set_confirmations(member_index, &true);

        Ok(())
    }
}
