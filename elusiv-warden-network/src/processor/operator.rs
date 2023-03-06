use crate::{
    error::ElusivWardenNetworkError,
    operator::WardenOperatorAccount,
    warden::{BasicWardenAccount, ElusivWardenID, Identifier},
};
use elusiv_types::UnverifiedAccountInfo;
use elusiv_utils::{guard, open_pda_account_with_associated_pubkey, pda_account};
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult};

pub fn register_warden_operator<'b>(
    operator: &AccountInfo<'b>,
    mut operator_account: UnverifiedAccountInfo<'_, 'b>,

    ident: Identifier,
    url: Identifier,
    jurisdiction: Option<u16>,
) -> ProgramResult {
    open_pda_account_with_associated_pubkey::<WardenOperatorAccount>(
        &crate::id(),
        operator,
        operator_account.get_unsafe_and_set_is_verified(),
        operator.key,
        None,
        None,
    )?;

    pda_account!(
        mut operator_account,
        WardenOperatorAccount,
        operator_account.get_safe()?
    );
    operator_account.set_ident(&ident);
    operator_account.set_url(&url);
    operator_account.set_jurisdiction(&jurisdiction.into());

    Ok(())
}

pub fn confirm_basic_warden_operation(
    operator: &AccountInfo,
    warden_account: &mut BasicWardenAccount,

    _warden_id: ElusivWardenID,
) -> ProgramResult {
    let mut warden = warden_account.get_warden();
    warden.is_operator_confirmed = true;
    match warden.config.operator.option() {
        Some(key) => {
            guard!(
                *operator.key == key,
                ElusivWardenNetworkError::InvalidSigner
            );
        }
        None => {
            warden.config.operator = Some(*operator.key).into();
        }
    }

    warden_account.set_warden(&warden);

    Ok(())
}
