use ark_ff::{ BigInteger256, bytes::ToBytes };
use num_bigint::BigUint;
use std::str::FromStr;

pub fn str_to_bytes(str: &str) -> Vec<u8> {
    let mut writer: Vec<u8> = vec![];
    str_to_bigint(str).write(&mut writer).unwrap();
    writer
}

pub fn str_to_bigint(str: &str) -> BigInteger256 {
    BigInteger256::try_from(BigUint::from_str(str).unwrap()).unwrap()
}