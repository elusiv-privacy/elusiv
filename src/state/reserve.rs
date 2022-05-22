//! Program account storing funds
 
use crate::macros::{pda_account, sized_account};

pub struct ReserveAccount {}
pda_account!(ReserveAccount, b"sol_reserve");
sized_account!(ReserveAccount, 1);