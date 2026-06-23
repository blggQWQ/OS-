use crate::sha256::sha256;

#[derive(Clone, Debug)]
pub struct HashTree {
    levels: Vec<Vec<[u8; 32]>>,
}

impl HashTree {
    pub fn build(blocks: &[Vec<u8>]) -> Self {
        let mut levels = Vec::new();
        let mut current: Vec<[u8; 32]> = blocks.iter().map(|block| sha256(block)).collect();
        levels.push(current.clone());

        while current.len() > 1 {
            let mut next = Vec::with_capacity((current.len() + 1) / 2);
            for pair in current.chunks(2) {
                let mut combined = Vec::with_capacity(pair.len() * 32);
                combined.extend_from_slice(&pair[0]);
                if pair.len() == 2 {
                    combined.extend_from_slice(&pair[1]);
                } else {
                    combined.extend_from_slice(&pair[0]);
                }
                next.push(sha256(&combined));
            }
            levels.push(next.clone());
            current = next;
        }

        Self { levels }
    }

    pub fn root_hash(&self) -> [u8; 32] {
        self.levels
            .last()
            .and_then(|level| level.first())
            .copied()
            .unwrap_or_else(|| sha256(&[]))
    }

    pub fn verify_block(&self, block_id: u64, block: &[u8]) -> bool {
        let mut idx = block_id as usize;
        let Some(leaves) = self.levels.first() else {
            return false;
        };
        if idx >= leaves.len() {
            return false;
        }

        let mut hash = sha256(block);
        if hash != leaves[idx] {
            return false;
        }

        for level in &self.levels[..self.levels.len().saturating_sub(1)] {
            let sibling_idx = if idx % 2 == 0 { idx + 1 } else { idx - 1 };
            let sibling = level.get(sibling_idx).copied().unwrap_or(hash);
            let mut combined = Vec::with_capacity(64);
            if idx % 2 == 0 {
                combined.extend_from_slice(&hash);
                combined.extend_from_slice(&sibling);
            } else {
                combined.extend_from_slice(&sibling);
                combined.extend_from_slice(&hash);
            }
            hash = sha256(&combined);
            idx /= 2;
        }

        hash == self.root_hash()
    }

    pub fn levels(&self) -> &[Vec<[u8; 32]>] {
        &self.levels
    }
}

