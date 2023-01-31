use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::pubkey::Pubkey;

pub trait BorshSerDeSized: BorshSerialize + BorshDeserialize {
    const SIZE: usize;

    fn override_slice(value: &Self, slice: &mut [u8]) -> Result<(), std::io::Error> {
        let vec = Self::try_to_vec(value)?;
        slice[..vec.len()].copy_from_slice(&vec[..]);
        Ok(())
    }
}

pub trait SizedType {
    const SIZE: usize;
}

pub trait BorshSerDeSizedEnum: BorshSerDeSized {
    fn len(variant_index: u8) -> usize;

    /// Deserializes an enum by reading only up to `len` bytes of the buffer
    fn deserialize_enum(buf: &mut &[u8]) -> std::io::Result<Self> {
        let len = Self::len(buf[0]) + 1;
        let v = Self::deserialize(&mut &buf[..std::cmp::min(len, buf.len())])?;
        Ok(v)
    }

    /// Deserializes an enum by reading all bytes of the buffer
    fn deserialize_enum_full(buf: &mut &[u8]) -> std::io::Result<Self> {
        let len = Self::len(buf[0]) + 1;
        let v = Self::deserialize(&mut &buf[..len])?;
        *buf = &buf[Self::SIZE - len..];
        Ok(v)
    }
}

#[allow(clippy::bool_to_int_with_if)]
pub const fn max(a: usize, b: usize) -> usize {
    [a, b][if a < b { 1 } else { 0 }]
}

#[macro_export]
macro_rules! impl_borsh_sized {
    ($ty: ty, $size: expr) => {
        impl BorshSerDeSized for $ty {
            const SIZE: usize = $size;
        }
    };
}

impl<E: BorshSerDeSized + Default + Copy, const N: usize> BorshSerDeSized for [E; N] {
    const SIZE: usize = E::SIZE * N;
}

impl_borsh_sized!(u8, 1);
impl_borsh_sized!(u16, 2);
impl_borsh_sized!(u32, 4);
impl_borsh_sized!(u64, 8);
impl_borsh_sized!(u128, 16);
impl_borsh_sized!(bool, 1);
impl_borsh_sized!(std::net::Ipv4Addr, 4);

#[derive(Copy, Clone, Debug)]
/// The advantage of `ElusivOption` over `Option` is fixed serialization length
pub enum ElusivOption<N> {
    Some(N),
    None,
}

impl<N: Clone + PartialEq> PartialEq<ElusivOption<N>> for ElusivOption<N> {
    fn eq(&self, other: &ElusivOption<N>) -> bool {
        self.option().eq(&other.option())
    }
}

impl<N> From<Option<N>> for ElusivOption<N> {
    fn from(o: Option<N>) -> Self {
        match o {
            Some(v) => ElusivOption::Some(v),
            None => ElusivOption::None,
        }
    }
}

impl<N: Clone> ElusivOption<N> {
    pub fn option(&self) -> Option<N> {
        match self {
            ElusivOption::Some(v) => Option::Some(v.clone()),
            ElusivOption::None => Option::None,
        }
    }
}

impl<T: BorshSerDeSized> BorshDeserialize for ElusivOption<T> {
    fn deserialize(buf: &mut &[u8]) -> std::io::Result<Self> {
        if buf[0] == 0 {
            *buf = &buf[<ElusivOption<T>>::SIZE..];
            Ok(ElusivOption::None)
        } else {
            *buf = &buf[1..];
            let v = T::deserialize(buf)?;
            Ok(ElusivOption::Some(v))
        }
    }
}

impl<T: BorshSerDeSized> BorshSerialize for ElusivOption<T> {
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        match self {
            ElusivOption::Some(v) => {
                writer.write_all(&[1])?;
                v.serialize(writer)
            }
            ElusivOption::None => {
                writer.write_all(&[0])?;
                writer.write_all(&vec![0; T::SIZE])?;
                Ok(())
            }
        }
    }
}

impl<T> Default for ElusivOption<T> {
    fn default() -> Self {
        ElusivOption::None
    }
}

impl<T: BorshSerDeSized> BorshSerDeSized for ElusivOption<T> {
    const SIZE: usize = 1 + T::SIZE;
}

impl BorshSerDeSized for Pubkey {
    const SIZE: usize = 32;
}

impl BorshSerDeSized for () {
    const SIZE: usize = 0;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate as elusiv_types;
    use crate::bytes::BorshSerDeSized;
    use elusiv_derive::BorshSerDeSized;

    #[test]
    fn test_max() {
        assert_eq!(max(1, 3), 3);
        assert_eq!(max(3, 1), 3);
    }

    #[derive(BorshDeserialize, BorshSerialize)]
    struct A {}
    impl_borsh_sized!(A, 11);

    #[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized)]
    struct B {
        a0: A,
        a1: A,
        a2: A,
    }

    #[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized)]
    enum C {
        A { a: A },
        B { b: B },
        AB { a: A, b: B },
    }

    #[test]
    fn test_borsh_ser_de_sized() {
        assert_eq!(A::SIZE, 11);
        assert_eq!(B::SIZE, 33);
        assert_eq!(C::SIZE, 11 + 33 + 1);
    }

    #[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, PartialEq, Debug)]
    enum TestEnum {
        A { v: [u64; 1] },
        B { v: [u64; 2] },
        C { v: [u64; 3], c: u8 },
    }

    #[test]
    fn test_enum_len() {
        assert_eq!(TestEnum::len(0), 8);
        assert_eq!(TestEnum::len(1), 16);
        assert_eq!(TestEnum::len(2), 25);
    }

    #[test]
    fn test_deserialize_enum() {
        let a = TestEnum::A { v: [333] };
        let mut data = a.try_to_vec().unwrap();
        data.extend(vec![255; TestEnum::SIZE - 8 - 1]);
        let buf = &mut &data[..];
        assert_eq!(TestEnum::deserialize_enum(buf).unwrap(), a);
        assert_eq!(TestEnum::deserialize_enum_full(buf).unwrap(), a);
    }

    #[test]
    #[should_panic]
    fn test_deserialize_enum_full() {
        let a = TestEnum::A { v: [333] };
        let data = a.try_to_vec().unwrap();
        let buf = &mut &data[..];
        _ = TestEnum::deserialize_enum_full(buf);
    }

    #[test]
    fn test_elusiv_option() {
        assert_eq!(ElusivOption::Some("abc").option(), Some("abc"));
        assert_eq!(ElusivOption::<u8>::None.option(), None);
    }
}
