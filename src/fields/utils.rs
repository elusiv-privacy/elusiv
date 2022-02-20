pub fn vec_to_array_32(v: Vec<u8>) -> [u8; 32] {
    let mut a = [0; 32];
    for i in 0..32 { a[i] = v[i]; }
    a
}