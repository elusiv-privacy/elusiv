use crate::{types::U256, bytes::{serialize_u256, unpack_u64, unpack_u256}};

#[derive(Clone, Copy, PartialEq)]
pub struct SendFinalizationRequest {
    pub amount: u64,
    pub recipient: U256,
}

impl SendFinalizationRequest {
    pub const SIZE: usize = 8 + 32;

    pub fn deserialize(data: &[u8]) -> SendFinalizationRequest {
        let (amount, data) = unpack_u64(data).unwrap();
        let (recipient, _) = unpack_u256(data).unwrap();

        SendFinalizationRequest { amount, recipient }
    }

    pub fn serialize(value: SendFinalizationRequest) -> Vec<u8> {
        let mut buffer = Vec::new();
        
        buffer.extend(value.amount.to_le_bytes());
        buffer.extend(serialize_u256(value.recipient));

        buffer
    }
}