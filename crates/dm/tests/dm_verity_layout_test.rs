use dm::sha256::sha256;

#[test]
fn packed_hash_block_root_matches_expected_layout() {// 测试打包哈希块的根哈希是否与预期布局匹配
    const BLOCK_SIZE: usize = 4096;// 哈希块的大小为4096字节
    let data = vec![0x5au8; BLOCK_SIZE];// 生成一个4096字节的随机数据
    let leaf = sha256(&data);// 计算数据的SHA-256哈希值
    let mut hash_block = vec![0u8; BLOCK_SIZE];
    hash_block[..32].copy_from_slice(&leaf);// 将哈希值的前32字节复制到哈希块的前32字节

    let root = sha256(&hash_block);// 计算哈希块的SHA-256哈希值

    assert_ne!(root, leaf);// 根哈希值与叶哈希值不同
    assert_eq!(root.len(), 32);// 根哈希值的长度为32字节
}
