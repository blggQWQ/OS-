use crate::error::{DmError, Result};
use crate::sha256::sha256;
use aes::cipher::{generic_array::GenericArray, BlockDecrypt, BlockEncrypt, KeyInit};
use aes::Aes256;

#[derive(Clone, Debug)]
pub struct DemoStreamCipher {
    key: Vec<u8>,
}

/// AES-256-XTS sector transform compatible with Linux's `aes-xts-plain64` mode.
#[derive(Clone)]
pub struct Aes256XtsCipher {
    data: Aes256,
    tweak: Aes256,
}

impl Aes256XtsCipher {
    pub const KEY_BYTES: usize = 64;
    pub const SECTOR_BYTES: usize = 512;

    pub fn new(key: &[u8]) -> Result<Self> {
        if key.len() != Self::KEY_BYTES {
            return Err(DmError::InvalidKey);
        }
        Ok(Self {
            data: Aes256::new(GenericArray::from_slice(&key[..32])),
            tweak: Aes256::new(GenericArray::from_slice(&key[32..])),
        })
    }

    pub fn encrypt_sector(&self, sector: u64, data: &mut [u8]) -> Result<()> {
        self.crypt_sector(sector, data, true)
    }

    pub fn decrypt_sector(&self, sector: u64, data: &mut [u8]) -> Result<()> {
        self.crypt_sector(sector, data, false)
    }

    fn crypt_sector(&self, sector: u64, data: &mut [u8], encrypt: bool) -> Result<()> {
        if data.is_empty() || data.len() % 16 != 0 {
            return Err(DmError::InvalidBlockSize {
                expected: Self::SECTOR_BYTES,
                actual: data.len(),
            });
        }

        // plain64 encodes the 64-bit sector number in the low half of a
        // little-endian 128-bit IV before encrypting it with the tweak key.
        let mut tweak = [0u8; 16];
        tweak[..8].copy_from_slice(&sector.to_le_bytes());
        self.tweak
            .encrypt_block(GenericArray::from_mut_slice(&mut tweak));

        for chunk in data.chunks_exact_mut(16) {
            xor_16(chunk, &tweak);
            if encrypt {
                self.data.encrypt_block(GenericArray::from_mut_slice(chunk));
            } else {
                self.data.decrypt_block(GenericArray::from_mut_slice(chunk));
            }
            xor_16(chunk, &tweak);
            gf_mul_x_le(&mut tweak);
        }
        Ok(())
    }
}

fn xor_16(block: &mut [u8], tweak: &[u8; 16]) {
    for (byte, mask) in block.iter_mut().zip(tweak) {
        *byte ^= mask;
    }
}

fn gf_mul_x_le(value: &mut [u8; 16]) {
    let mut carry = 0u8;
    for byte in value.iter_mut() {
        let next = *byte >> 7;
        *byte = (*byte << 1) | carry;
        carry = next;
    }
    if carry != 0 {
        value[0] ^= 0x87;
    }
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
