use solana_program::pubkey::Pubkey;

pub struct ElusivBasicWarden {
    pub key: Pubkey,
}

pub struct ElusivFullWarden {
    pub key: Pubkey,
    pub apae_key: Pubkey,
}

pub type ElusivWardenID = u32;