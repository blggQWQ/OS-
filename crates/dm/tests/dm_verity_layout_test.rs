use dm::sha256::sha256;

#[test]
fn packed_hash_block_root_matches_expected_layout() {
    // 测试dm-verity哈希树的最简布局：只有1个数据块时的哈希树结构
    // 结构：data → leaf_hash → hash_block(4096字节) → root_hash
    // hash_block的布局：前32字节放leaf哈希，剩余填零
    // 这是dm-verity的标准格式：一个4096字节的哈希块可以装128个哈希值(4096/32)
    const BLOCK_SIZE: usize = 4096;
    let data = vec![0x5au8; BLOCK_SIZE]; // 4096字节的数据块
    let leaf = sha256(&data); // 叶子哈希 = SHA256(数据)
    let mut hash_block = vec![0u8; BLOCK_SIZE];
    hash_block[..32].copy_from_slice(&leaf); // 哈希块前32字节放叶子哈希，其余全零

    let root = sha256(&hash_block); // 根哈希 = SHA256(整个哈希块)

    assert_ne!(root, leaf); // 根哈希 ≠ 叶子哈希（输入不同，输出不同）
    assert_eq!(root.len(), 32); // SHA-256输出固定32字节
}

#[test]
fn two_data_blocks_pack_into_one_hash_block() {
    // 测试2个数据块的哈希树布局
    // 两个叶子哈希都放在同一个4096字节的哈希块里：
    //   [leaf0(32字节) | leaf1(32字节) | 0 | 0 | ...]
    // 根哈希 = SHA256(整个哈希块)
    const BLOCK_SIZE: usize = 4096;
    let data0 = vec![0x5au8; BLOCK_SIZE];
    let data1 = vec![0xa5u8; BLOCK_SIZE]; // 不同于data0
    let leaf0 = sha256(&data0);
    let leaf1 = sha256(&data1);

    let mut hash_block = vec![0u8; BLOCK_SIZE];
    hash_block[..32].copy_from_slice(&leaf0); // 前32字节放leaf0
    hash_block[32..64].copy_from_slice(&leaf1); // 接下来32字节放leaf1

    let root = sha256(&hash_block);

    assert_ne!(leaf0, leaf1); // 不同数据 → 不同哈希
    assert_ne!(root, leaf0); // 根哈希 ≠ 任一叶子哈希
    assert_ne!(root, leaf1);
    assert_eq!(root.len(), 32);
}

#[test]
fn hash_block_fits_128_hashes() {
    // 测试一个4096字节的哈希块可以装128个SHA-256哈希值
    // 这是dm-verity的核心设计：4096 / 32 = 128
    // 当数据块 ≤ 128 时，只需一层哈希树
    const BLOCK_SIZE: usize = 4096;
    const HASH_SIZE: usize = 32;
    const HASHES_PER_BLOCK: usize = BLOCK_SIZE / HASH_SIZE; // = 128

    // 模拟128个数据块的叶子哈希
    let mut hash_block = vec![0u8; BLOCK_SIZE];
    for i in 0..HASHES_PER_BLOCK {
        let data = vec![i as u8; BLOCK_SIZE]; // 每块数据不同
        let leaf = sha256(&data);
        hash_block[i * HASH_SIZE..(i + 1) * HASH_SIZE].copy_from_slice(&leaf);
    }

    let root = sha256(&hash_block);
    assert_eq!(root.len(), 32);

    // 验证相邻哈希不同（不同数据→不同哈希）
    let first_hash = &hash_block[..HASH_SIZE];
    let second_hash = &hash_block[HASH_SIZE..2 * HASH_SIZE];
    assert_ne!(first_hash, second_hash);
}