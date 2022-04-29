use elusiv::error::ElusivError;

/// Minimum storage Merkle Tree (MMT)
/// - append only (left to right)
/// - requires n storage + a pointer for a MT of height n
pub struct MMT<T, const N: usize> {
    pub pointer: u64,
    pub hashes: [T; N],
    pub root: T,
}

impl<T, const N: usize> MMT<T, N> {
    pub fn append<Hash>(&mut self, value: T, hash: Hash) -> Result<(), ElusivError>
    where Hash: Fn(Vec<T>) -> T
    {
        Ok(())
    }
}