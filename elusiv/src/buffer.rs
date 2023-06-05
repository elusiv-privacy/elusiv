use crate::error::ElusivError;
use elusiv_types::BorshSerDeSized;

#[allow(clippy::len_without_is_empty)]
pub trait RingBuffer<'a, N: BorshSerDeSized> {
    const CAPACITY: usize;

    fn len(&self) -> usize;
    fn set_len(&mut self, len: usize);

    fn ptr(&self) -> usize;
    fn set_ptr(&mut self, ptr: usize);

    fn set_value(&mut self, index: usize, value: &N);

    fn contains(&self, value: &N) -> bool;
    fn find_position(&self, value: &N) -> Option<usize>;

    fn push(&mut self, value: &N) {
        let ptr = self.ptr() % Self::CAPACITY;
        self.set_ptr((ptr + 1) % Self::CAPACITY);

        self.set_value(ptr, value);

        let len = self.len();
        self.set_len(std::cmp::min(len + 1, Self::CAPACITY))
    }

    fn try_insert(&mut self, value: &N) -> Result<(), ElusivError> {
        if self.contains(value) {
            return Err(ElusivError::DuplicateValue);
        }

        self.push(value);

        Ok(())
    }
}

macro_rules! buffer_account {
    ($ident: ident, $ty: ty, $size: expr $(,)?) => {
        #[allow(dead_code)]
        #[crate::macros::elusiv_account]
        pub struct $ident {
            #[no_getter]
            #[no_setter]
            pda_data: PDAAccountData,

            #[no_getter]
            values: [$ty; $size as usize],
            length: u32,
            pointer: u32,
        }

        #[cfg(test)]
        const_assert!($size < u32::MAX as usize);

        impl<'a> crate::buffer::RingBuffer<'a, $ty> for $ident<'a> {
            const CAPACITY: usize = $size;

            fn len(&self) -> usize {
                self.get_length() as usize
            }

            fn set_len(&mut self, len: usize) {
                self.set_length(&len.try_into().unwrap())
            }

            fn ptr(&self) -> usize {
                self.get_pointer() as usize
            }

            fn set_ptr(&mut self, ptr: usize) {
                self.set_pointer(&ptr.try_into().unwrap())
            }

            fn set_value(&mut self, index: usize, value: &$ty) {
                self.set_values(index, value);
            }

            fn contains(&self, value: &$ty) -> bool {
                let len = self.len();
                if len == 0 {
                    return false;
                }

                crate::bytes::contains(
                    value,
                    &self.values[..len * <$ty as elusiv_types::bytes::BorshSerDeSized>::SIZE],
                )
            }

            fn find_position(&self, value: &$ty) -> Option<usize> {
                let len = self.len();
                if len == 0 {
                    return None;
                }

                crate::bytes::find(
                    value,
                    &self.values[..len * <$ty as elusiv_types::bytes::BorshSerDeSized>::SIZE],
                    len,
                )
            }
        }
    };
}

pub(crate) use buffer_account;

#[cfg(test)]
mod test {
    use crate::{buffer::RingBuffer, error::ElusivError, macros::zero_program_account};
    use elusiv_types::accounts::PDAAccountData;

    const TEST_BUFFER_ACCOUNT_SIZE: usize = 128;
    buffer_account!(TestBufferAccount, u32, TEST_BUFFER_ACCOUNT_SIZE);

    #[test]
    fn test_contains() {
        zero_program_account!(mut buffer, TestBufferAccount);

        assert!(!buffer.contains(&0));

        buffer.set_len(TestBufferAccount::CAPACITY);
        for i in 0..TestBufferAccount::CAPACITY as u32 {
            buffer.set_values(i as usize, &i);
        }

        for i in 0..TestBufferAccount::CAPACITY as u32 {
            assert!(buffer.contains(&i));
        }

        assert!(!buffer.contains(&(TestBufferAccount::CAPACITY as u32 + 1)));
    }

    #[test]
    fn test_find_position() {
        zero_program_account!(mut buffer, TestBufferAccount);

        for i in 0..TestBufferAccount::CAPACITY as u32 {
            assert!(buffer.find_position(&i).is_none());
            buffer.try_insert(&i).unwrap();
            assert_eq!(buffer.find_position(&i).unwrap(), i as usize);
        }
    }

    #[test]
    fn test_push() {
        zero_program_account!(mut buffer, TestBufferAccount);

        assert_eq!(buffer.len(), 0);
        assert_eq!(buffer.ptr(), 0);

        for i in 1..=TestBufferAccount::CAPACITY {
            buffer.push(&(i as u32));

            assert_eq!(buffer.len(), i);
            assert_eq!(buffer.ptr(), i % TestBufferAccount::CAPACITY);
        }

        buffer.push(&0);
        assert_eq!(buffer.len(), TestBufferAccount::CAPACITY);
        assert_eq!(buffer.ptr(), 1);
    }

    #[test]
    fn test_try_insert() {
        zero_program_account!(mut buffer, TestBufferAccount);

        for i in 1..=TestBufferAccount::CAPACITY {
            assert_eq!(buffer.try_insert(&(i as u32)), Ok(()));

            assert_eq!(buffer.len(), i);
            assert_eq!(buffer.ptr(), i % TestBufferAccount::CAPACITY);
        }

        // Duplicates get rejected
        let values = buffer.values.to_vec();
        for i in 1..=TestBufferAccount::CAPACITY {
            assert_eq!(
                buffer.try_insert(&(i as u32)),
                Err(ElusivError::DuplicateValue)
            );

            assert_eq!(buffer.len(), TestBufferAccount::CAPACITY);
            assert_eq!(buffer.ptr(), 0);
            assert_eq!(buffer.values, values);
        }

        // FIFO
        for i in 1..=TestBufferAccount::CAPACITY {
            assert!(buffer.contains(&(i as u32)));
            assert_eq!(
                buffer.try_insert(&((i + TestBufferAccount::CAPACITY) as u32)),
                Ok(())
            );
            assert!(!buffer.contains(&(i as u32)));

            assert_eq!(buffer.len(), TestBufferAccount::CAPACITY);
            assert_eq!(buffer.ptr(), i % TestBufferAccount::CAPACITY);
        }
    }
}
