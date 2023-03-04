use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::pubkey::Pubkey;

pub trait BorshSerDeSized: BorshSerialize + BorshDeserialize {
    const SIZE: usize;
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

/// The advantage of [`ElusivOption`] over [`Option`] is the fixed serialization length
#[derive(Copy, Clone)]
#[cfg_attr(any(test, feature = "elusiv-client"), derive(Debug))]
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
