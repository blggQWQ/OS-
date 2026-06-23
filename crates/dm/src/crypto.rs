use crate::error::{DmError, Result};
use crate::sha256::sha256;

#[derive(Clone, Debug)]
pub struct DemoStreamCipher {
    key: Vec<u8>,
}

impl DemoStreamCipher {
    pub fn new(key: &[u8]) -> Result<Self> {
        if key.is_empty() {
            return Err(DmError::InvalidKey);
        }
        Ok(Self { key: key.to_vec() })
    }

    pub fn apply(&self, block_id: u64, input: &[u8], output: &mut [u8]) -> Result<()> {
        if input.len() != output.len() {
            return Err(DmError::InvalidBlockSize {
                expected: input.len(),
                actual: output.len(),
            });
        }

        let mut offset = 0usize;
        let mut counter = 0u64;
        while offset < input.len() {
            let stream = self.keystream_block(block_id, counter);
            for byte in stream {
                if offset == input.len() {
                    break;
                }
                output[offset] = input[offset] ^ byte;
                offset += 1;
            }
            counter += 1;
        }
        Ok(())
    }

    fn keystream_block(&self, block_id: u64, counter: u64) -> [u8; 32] {
        let mut seed = Vec::with_capacity(self.key.len() + 16);
        seed.extend_from_slice(&self.key);
        seed.extend_from_slice(&block_id.to_le_bytes());
        seed.extend_from_slice(&counter.to_le_bytes());
        sha256(&seed)
    }
}

