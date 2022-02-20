use super::fields::scalar::*;
use super::state::TREE_HEIGHT;

/// Returns a tree node as a Scalar object
///
/// ### Arguments
/// 
/// * `store` - The byte buffer (required size: 2^{TREE_HEIGHT + 1} - 1)
/// * `layer` - in [0; TREE_HEIGHT]
/// * `index` - in [0; 2^{layer})
pub fn node(store: &[u8], layer: usize, index: usize) -> Scalar {
    let s_index = store_index(layer, index);
    from_bytes_le_mont(&store[s_index..s_index + 32])
}

/// Returns the neighbour node (aka the other node of a pair of nodes in a layer)
pub fn neighbour(store: &[u8], layer: usize, index: usize) -> Scalar {
    let index = if index % 2 == 0 { index + 1 } else { index - 1};
    node(store, layer, index)
}

/// Inserts the little endian bytes as a node into the tree
fn set_node(store: &mut [u8], layer: usize, index: usize, bytes: &[u8]) {
    let s_index = store_index(layer, index);

    for (i, &byte) in bytes.iter().enumerate() {
        store[s_index + i] = byte;
    }
}

pub fn opening(store: &[u8], index: usize) -> [u8; TREE_HEIGHT * 32] {
    let mut bytes = [0; TREE_HEIGHT * 32];
    let mut index = index;

    for i in 0..TREE_HEIGHT {
        let layer = TREE_HEIGHT - i;
        let n_index = if index % 2 == 0 { index + 1 } else { index - 1};
        let s_index = store_index(layer, n_index);
        for j in 0..32 {
            bytes[i * 32 + j] = store[s_index + j];
        }
        index = index >> 1;
    }

    bytes
}

/// Inserts the hashes (incl. leaf & root) into the tree
/// 
/// ### Arguments
/// 
/// * `store` - The byte buffer (required size: 2^{TREE_HEIGHT + 1} - 1)
/// * `hashes` - Array of 32 byte arrays; length TREE_HEIGHT + 1 (all layers incl. leaf & root)
/// * `leaf_index` - in [0; 2^{TREE_HEIGHT})
pub fn insert_hashes(store: &mut [u8], hashes: [[u8; 32]; TREE_HEIGHT + 1], leaf_index: usize) {
    for (i, hash) in hashes.iter().enumerate() {
        let layer = TREE_HEIGHT - i;
        let layer_index = leaf_index >> i;
        set_node(store, layer, layer_index, hash);
    }
}

/// Converts a layer index into an byte array (!) index
/// 
/// ### Arguments
/// 
/// * `layer` - in [0; TREE_HEIGHT]
/// * `index` - in [0; 2^{TREE_HEIGHT})
pub fn store_index(layer: usize, index: usize) -> usize {
    // Equal to: ((2^{layer} - 1) + index) * 32
    ((1 << layer) - 1 + index) << 5
}

/// Returns the size of a tree (not byte size!)
fn size_of_tree(height: usize) -> usize {
    (1 << (height + 1)) - 1
}

/// Returns the size of a layer (not byte size!)
fn size_of_layer(layer: usize) -> usize {
    1 << layer
}

pub fn generate_vector<Limb: Copy>(height: usize, limbing_power: usize, zero_value: Limb) -> Vec<Limb> {
    vec![zero_value; size_of_tree(height) << limbing_power]
}

pub fn initialize_store<Hash>(store: &mut [u8], zero_value: Scalar, hash: Hash)
where Hash: Fn(Scalar, Scalar) -> Scalar
{
    let mut value = zero_value;
    for layer in (0..=TREE_HEIGHT).rev() {
        let bytes = to_bytes_le_mont(value);

        for j in 0..size_of_layer(layer) {
            set_node(store, layer, j, &bytes);
        }

        value = hash(value, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_ff::*;

    #[test]
    fn test_store_index() {
        assert_eq!(0, store_index(0, 0));
        assert_eq!(1 * 32, store_index(1, 0));
        assert_eq!(2 * 32, store_index(1, 1));
        assert_eq!(3 * 32, store_index(2, 0));
        assert_eq!(4 * 32, store_index(2, 1));
        assert_eq!(5 * 32, store_index(2, 2));
        assert_eq!(6 * 32, store_index(2, 3));
        assert_eq!(7 * 32, store_index(2, 4));
    }

    #[test]
    fn test_size_of_tree() {
        assert_eq!(1, size_of_tree(0));
        assert_eq!(3, size_of_tree(1));
        assert_eq!(7, size_of_tree(2));
        assert_eq!(15, size_of_tree(3));
    }

    #[test]
    fn test_size_of_layer() {
        assert_eq!(1, size_of_layer(0));
        assert_eq!(2, size_of_layer(1));
        assert_eq!(4, size_of_layer(2));
        assert_eq!(8, size_of_layer(3));
    }

    fn set(data: &mut [u8], layer: usize, index: usize, value: Scalar) {
        let bytes = to_bytes_le_mont(value);
        for (i, &byte) in bytes.iter().enumerate() {
            data[store_index(layer, index) + i] = byte;
        }
    }

    #[test]
    fn test_node() {
        let mut data = vec![0; size_of_tree(TREE_HEIGHT) * 32];

        assert_eq!(Scalar::zero(), node(&data, 0, 0));

        let s = from_str_10("1");
        set(&mut data, 0, 0, s);
        assert_eq!(s, node(&data, 0, 0));

        set(&mut data, 1, 0, from_str_10("255"));
        assert_eq!(from_str_10("255"), node(&data, 1, 0));

        assert_eq!(Scalar::zero(), node(&data, 1, 1));

        set(&mut data, 1, 1, from_str_10("12345678910111213141516"));
        assert_eq!(from_str_10("12345678910111213141516"), node(&data, 1, 1));
    }

    #[test]
    fn test_neighbour() {
        let mut data = vec![0; size_of_tree(TREE_HEIGHT) * 32];

        set(&mut data, 5, 0, from_str_10("999"));
        set(&mut data, 5, 1, from_str_10("100"));
        set(&mut data, 9, 10, from_str_10("666"));
        set(&mut data, 11, 230, from_str_10("123456789"));

        assert_eq!(from_str_10("999"), neighbour(&data, 5, 1));
        assert_eq!(from_str_10("100"), neighbour(&data, 5, 0));
        assert_eq!(from_str_10("666"), neighbour(&data, 9, 11));
        assert_eq!(from_str_10("123456789"), neighbour(&data, 11, 231));
    }

    #[test]
    fn test_set_node() {
        let mut data = vec![0; size_of_tree(TREE_HEIGHT) * 32];
        let scalar = from_str_10("12345678987654321");
        let bytes = to_bytes_le_repr(scalar);

        set_node(&mut data, 11, 222, &bytes);

        assert_eq!(
            data[store_index(11, 222)..store_index(11, 222) + 32],
            bytes
        );
    }

    #[test]
    fn test_insert_hashes() {
        let mut data = vec![0; size_of_tree(TREE_HEIGHT) * 32];
        let mut hashes = [[0; 32]; TREE_HEIGHT + 1];
        let index = 333;
        for i in 0..TREE_HEIGHT + 1 {
            let mut bytes = [0 as u8; 32];
            bytes[0] = (i + 1) as u8;
            hashes[i] = bytes;
        }

        insert_hashes(&mut data, hashes, index);

        for i in 0..TREE_HEIGHT + 1 {
            assert_eq!(
                to_bytes_le_mont(node(&data, TREE_HEIGHT - i, index >> i)),
                hashes[i]
            )
        }
    }
}