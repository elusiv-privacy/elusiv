use super::poseidon::*;
use super::scalar::*;
use super::state::TREE_HEIGHT;

fn get_store_index(layer: usize, index: usize) -> usize {
    (start_of_layer(layer) + index) * 32
}

pub fn get_node(store: &[u8], layer: usize, index: usize) -> Scalar {
    let s_index = get_store_index(layer, index);
    from_bytes_le(&store[s_index..s_index + 32])
}

fn set_node(store: &mut [u8], layer: usize, index: usize, bytes: &[u8]) {
    if index > size_of_layer(layer) { return; }
    let s_index = get_store_index(layer, index);

    for (i, &byte) in bytes.iter().enumerate() {
        store[s_index + i] = byte;
    }
}

pub fn get_leaf(store: &[u8], index: usize) -> Scalar {
    get_node(&store, TREE_HEIGHT, index)
}

fn get_path(index: usize) -> Vec<usize> {
    let mut path = Vec::new();
    let mut index = index;

    for _ in 0..=TREE_HEIGHT {
        path.push(if index % 2 == 0 { index + 1 } else { index - 1 });
        index = index / 2;
    }

    path
}

pub fn get_neighbour(store: &[u8], layer_inverse: usize, leaf_index: usize) -> Scalar {
    let path = get_path(leaf_index);
    get_node(store, TREE_HEIGHT - layer_inverse, path[layer_inverse])
}

pub fn insert_hashes(store: &mut [u8], hashes: [[u8; 32]; TREE_HEIGHT + 1], index: usize) {
    for i in 0..=TREE_HEIGHT {
        let layer = TREE_HEIGHT - i;
        let layer_index = index >> i;
        set_node(store, layer, layer_index, &hashes[i]);
    }
}

pub fn initialize_store<Hash>(store: &mut [u8], zero_value: Scalar, hash: Hash)
where Hash: Fn(Scalar, Scalar) -> Scalar
{
    let mut value = zero_value;
    for layer in (0..=TREE_HEIGHT).rev() {
        let bytes = to_bytes_le(value);

        for j in 0..size_of_layer(layer) {
            set_node(store, layer, j, &bytes);
        }

        value = hash(value, value);
    }
}

fn start_of_layer(layer: usize) -> usize {
    (1 << layer) - 1
}

fn size_of_tree(height: usize) -> usize {
    (1 << (height + 1)) - 1
}

fn size_of_layer(layer: usize) -> usize {
    1 << layer
}

pub fn generate_vector<Limb: Copy>(height: usize, limbing_power: usize, zero_value: Limb) -> Vec<Limb> {
    vec![zero_value; size_of_tree(height) << limbing_power]
}