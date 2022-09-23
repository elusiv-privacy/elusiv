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

macro_rules! warden_network {
    ($ty: ident, $type: expr, $size: expr) => {
        pub struct $ty {}

        impl WardenNetwork for $ty {
            const TYPE: WardenNetworkType = $type;
            const SIZE: WardenNetworkSize = $size;
        }
    };
}

pub const BASIC_WARDEN_GENESIS_NETWORK_SIZE_LIMIT: usize = 1024;

warden_network! {
    ElusivBasicWardenGenesisNetwork,
    WardenNetworkType::Mixed,
    WardenNetworkSize::Dynamic(0, BASIC_WARDEN_GENESIS_NETWORK_SIZE_LIMIT)
}

pub const FULL_WARDEN_GENESIS_NETWORK_SIZE: usize = 6;

warden_network! {
    ElusivFullWardenGenesisNetwork,
    WardenNetworkType::Full,
    WardenNetworkSize::Fixed(FULL_WARDEN_GENESIS_NETWORK_SIZE)
}