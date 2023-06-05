use crate::warden::{ElusivWardenID, Quote, QuoteEnd, QuoteStart, WardenRegion};
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
    #[no_getter]
    #[no_setter]
    pda_data: PDAAccountData,

    members_count: u32,
    members: [ElusivWardenID; ElusivBasicWardenNetwork::SIZE.max()],
    features: [BasicWardenFeatures; ElusivBasicWardenNetwork::SIZE.max()],
    tokens: [[bool; TOKENS.len()]; ElusivBasicWardenNetwork::SIZE.max()],
    region: [WardenRegion; ElusivBasicWardenNetwork::SIZE.max()],
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
    #[no_getter]
    #[no_setter]
    pda_data: PDAAccountData,

    members_count: u32,
    apa_key: ElusivOption<Pubkey>,

    members: [ElusivWardenID; ElusivApaWardenNetwork::SIZE.max()],
    quote_starts: [QuoteStart; ElusivApaWardenNetwork::SIZE.max()],
    quote_ends: [ElusivOption<QuoteEnd>; ElusivApaWardenNetwork::SIZE.max()],
    exchange_keys: [Pubkey; ElusivApaWardenNetwork::SIZE.max()],
    confirmations: [bool; ElusivApaWardenNetwork::SIZE.max()],
}

impl<'a> ApaWardenNetworkAccount<'a> {
    pub fn is_application_phase(&self) -> bool {
        !(0..ElusivApaWardenNetwork::SIZE.max()).all(|i| {
            let opt: Option<QuoteEnd> = self.get_quote_ends(i).option();
            opt.is_some()
        })
    }

    pub fn is_confirmation_phase(&self) -> bool {
        !self.is_application_phase() && !self.is_confirmed()
    }

    pub fn is_confirmed(&self) -> bool {
        if self.is_application_phase() {
            return false;
        }

        (0..self.get_members_count() as usize).all(|i| self.get_confirmations(i))
    }

    pub fn start_application(
        &mut self,
        warden_id: ElusivWardenID,
        quote_start: &QuoteStart,
    ) -> Result<u32, ProgramError> {
        guard!(
            self.is_application_phase(),
            ElusivWardenNetworkError::WardenRegistrationError
        );

        let members_count = self.get_members_count();
        self.set_members_count(&(members_count + 1));

        self.set_members(members_count as usize, &warden_id);
        self.set_quote_starts(members_count as usize, quote_start);
        self.set_exchange_keys(
            members_count as usize,
            &Pubkey::new_from_array(quote_start.user_data_bytes()),
        );

        Ok(members_count)
    }

    pub fn complete_application(
        &mut self,
        warden_id: ElusivWardenID,
        quote_end: QuoteEnd,
    ) -> Result<(), ProgramError> {
        guard!(
            self.is_application_phase(),
            ElusivWardenNetworkError::WardenRegistrationError
        );

        let member_index = (0..self.get_members_count() as usize)
            .map(|i| self.get_members(i))
            .find(|member| member == &warden_id)
            .ok_or(ElusivWardenNetworkError::WardenRegistrationError)?;
        self.set_quote_ends(member_index as usize, &Some(quote_end).into());

        Ok(())
    }

    pub fn get_all_quotes(&self) -> Vec<Quote> {
        (0..self.get_members_count() as usize)
            .filter_map(|i| {
                let start = self.get_quote_starts(i);
                let end: Option<QuoteEnd> = self.get_quote_ends(i).option();

                end.map(|end| start.join(&end))
            })
            .collect()
    }

    pub fn confirmation_message(&self) -> [u8; 32] {
        let quotes = self.get_all_quotes();
        let quote_bytes = quotes
            .iter()
            .map(|quote| quote.0.as_slice())
            .collect::<Vec<&[u8]>>();
        let hash = solana_program::hash::hashv(&quote_bytes);
        hash.to_bytes()
    }

    pub fn confirm_others(
        &mut self,
        member_index: usize,
        signer: &Pubkey,
        confirmation_message: &[u8],
    ) -> ProgramResult {
        guard!(
            self.is_confirmation_phase(),
            ElusivWardenNetworkError::NotInConfirmationPhase
        );
        guard!(
            self.get_exchange_keys(member_index) == *signer,
            ElusivWardenNetworkError::SignerAndWardenIdMismatch
        );
        guard!(
            self.confirmation_message() == confirmation_message,
            ElusivWardenNetworkError::InvalidConfirmationMessage
        );
        guard!(
            !self.get_confirmations(member_index),
            ElusivWardenNetworkError::WardenAlreadyConfirmed
        );

        self.set_confirmations(member_index, &true);

        Ok(())
    }
}
