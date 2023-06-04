use elusiv_proc_macros::elusiv_account;
use elusiv_types::{ChildAccount, ElusivOption, PDAAccountData};
use solana_program::pubkey::Pubkey;

pub struct VKeyChildAccount;

impl ChildAccount for VKeyChildAccount {
    const INNER_SIZE: usize = 0;
}

/// Account used for storing a single immutable [`VerifyingKey`]
#[elusiv_account(parent_account: { child_account_count: 2, child_account: VKeyChildAccount }, eager_type: true)]
pub struct VKeyAccount {
    #[no_getter]
    #[no_setter]
    pda_data: PDAAccountData,
    pubkeys: [ElusivOption<Pubkey>; 2],

    pub public_inputs_count: u32,
    pub authority: ElusivOption<Pubkey>,
    pub is_frozen: bool,
    pub version: u32,
}

impl<'a, 'b, 't> VKeyAccount<'a, 'b, 't> {
    pub fn is_setup(&self) -> bool {
        self.get_version() != 0
    }
}
