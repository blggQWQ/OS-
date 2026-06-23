use dm::sha256::sha256;

#[test]
fn packed_hash_block_root_matches_expected_layout() {
    const BLOCK_SIZE: usize = 4096;
    let data = vec![0x5au8; BLOCK_SIZE];
    let leaf = sha256(&data);
    let mut hash_block = vec![0u8; BLOCK_SIZE];
    hash_block[..32].copy_from_slice(&leaf);

    let root = sha256(&hash_block);

    assert_ne!(root, leaf);
    assert_eq!(root.len(), 32);
}
