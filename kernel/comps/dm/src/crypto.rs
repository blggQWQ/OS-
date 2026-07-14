// SPDX-License-Identifier: MPL-2.0

use aes::{
    Aes256,
    cipher::{BlockDecrypt, BlockEncrypt, NewBlockCipher, generic_array::GenericArray},
};
#[cfg(all(target_arch = "x86_64", dm_aes_ni))]
use ostd::{arch::cpu::context::FpuContext, irq, task::disable_preempt};

use crate::DmError;

const AES_BLOCK_SIZE: usize = 16;
const XTS_KEY_SIZE: usize = 64;

pub(crate) fn hardware_aes_available() -> bool {
    #[cfg(all(target_arch = "x86_64", dm_aes_ni))]
    {
        ostd::arch::cpu::aesni::is_available()
    }
    #[cfg(not(all(target_arch = "x86_64", dm_aes_ni)))]
    {
        false
    }
}

/// Encrypts and decrypts sectors with AES-256-XTS.
pub(crate) struct AesXtsCipher {
    data_cipher: Aes256Backend,
    tweak_cipher: Aes256Backend,
}

enum Aes256Backend {
    Portable(Aes256),
    #[cfg(all(target_arch = "x86_64", dm_aes_ni))]
    Hardware([[u8; AES_BLOCK_SIZE]; 15]),
}

impl Aes256Backend {
    fn new(key: &[u8]) -> Self {
        #[cfg(all(target_arch = "x86_64", dm_aes_ni))]
        if hardware_aes_available() {
            let key: &[u8; 32] = key.try_into().unwrap();
            return Self::Hardware(expand_key_256(key));
        }
        Self::Portable(Aes256::new(GenericArray::from_slice(key)))
    }

    fn encrypt(&self, block: &mut [u8; AES_BLOCK_SIZE]) {
        match self {
            Self::Portable(cipher) => cipher.encrypt_block(GenericArray::from_mut_slice(block)),
            #[cfg(all(target_arch = "x86_64", dm_aes_ni))]
            Self::Hardware(keys) => ostd::arch::cpu::aesni::encrypt_block_256(block, keys),
        }
    }

    fn decrypt(&self, block: &mut [u8; AES_BLOCK_SIZE]) {
        match self {
            Self::Portable(cipher) => cipher.decrypt_block(GenericArray::from_mut_slice(block)),
            #[cfg(all(target_arch = "x86_64", dm_aes_ni))]
            Self::Hardware(keys) => ostd::arch::cpu::aesni::decrypt_block_256(block, keys),
        }
    }
}

#[cfg(all(target_arch = "x86_64", dm_aes_ni))]
fn expand_key_256(key: &[u8; 32]) -> [[u8; AES_BLOCK_SIZE]; 15] {
    let mut words = [[0u8; 4]; 60];
    for (word, bytes) in words[..8].iter_mut().zip(key.chunks_exact(4)) {
        word.copy_from_slice(bytes);
    }

    let mut round_constant = 1u8;
    for index in 8..words.len() {
        let mut temporary = words[index - 1];
        if index % 8 == 0 {
            temporary.rotate_left(1);
            temporary
                .iter_mut()
                .for_each(|byte| *byte = aes_sbox(*byte));
            temporary[0] ^= round_constant;
            round_constant = gf_mul(round_constant, 2);
        } else if index % 8 == 4 {
            temporary
                .iter_mut()
                .for_each(|byte| *byte = aes_sbox(*byte));
        }
        for byte in 0..4 {
            words[index][byte] = words[index - 8][byte] ^ temporary[byte];
        }
    }

    let mut keys = [[0u8; AES_BLOCK_SIZE]; 15];
    for (round_key, round_words) in keys.iter_mut().zip(words.chunks_exact(4)) {
        for (destination, word) in round_key.chunks_exact_mut(4).zip(round_words) {
            destination.copy_from_slice(word);
        }
    }
    keys
}

#[cfg(all(target_arch = "x86_64", dm_aes_ni))]
fn aes_sbox(value: u8) -> u8 {
    let inverse = if value == 0 {
        0
    } else {
        let mut result = 1u8;
        let mut base = value;
        let mut exponent = 254u8;
        while exponent != 0 {
            if exponent & 1 != 0 {
                result = gf_mul(result, base);
            }
            base = gf_mul(base, base);
            exponent >>= 1;
        }
        result
    };
    inverse
        ^ inverse.rotate_left(1)
        ^ inverse.rotate_left(2)
        ^ inverse.rotate_left(3)
        ^ inverse.rotate_left(4)
        ^ 0x63
}

#[cfg(all(target_arch = "x86_64", dm_aes_ni))]
fn gf_mul(mut left: u8, mut right: u8) -> u8 {
    let mut product = 0u8;
    while right != 0 {
        if right & 1 != 0 {
            product ^= left;
        }
        left = (left << 1) ^ if left & 0x80 != 0 { 0x1b } else { 0 };
        right >>= 1;
    }
    product
}

impl AesXtsCipher {
    /// Creates an AES-256-XTS cipher from a 512-bit key.
    pub(crate) fn new(key: &[u8]) -> Result<Self, DmError> {
        if key.len() != XTS_KEY_SIZE {
            return Err(DmError::InvalidKey);
        }

        let (data_key, tweak_key) = key.split_at(XTS_KEY_SIZE / 2);
        Ok(Self {
            data_cipher: Aes256Backend::new(data_key),
            tweak_cipher: Aes256Backend::new(tweak_key),
        })
    }

    /// Encrypts one or more complete 512-byte sectors in place.
    pub(crate) fn encrypt(&self, first_sector: u64, data: &mut [u8]) -> Result<(), DmError> {
        self.transform(first_sector, data, XtsDirection::Encrypt)
    }

    /// Decrypts one or more complete 512-byte sectors in place.
    pub(crate) fn decrypt(&self, first_sector: u64, data: &mut [u8]) -> Result<(), DmError> {
        self.transform(first_sector, data, XtsDirection::Decrypt)
    }

    fn transform(
        &self,
        first_sector: u64,
        data: &mut [u8],
        direction: XtsDirection,
    ) -> Result<(), DmError> {
        if !data.len().is_multiple_of(aster_block::SECTOR_SIZE) {
            return Err(DmError::UnalignedIo);
        }
        let sector_count = data.len() / aster_block::SECTOR_SIZE;
        if sector_count != 0 {
            first_sector
                .checked_add((sector_count - 1) as u64)
                .ok_or(DmError::OutOfRange)?;
        }

        // AES-NI uses XMM registers. Save the interrupted context, keep the
        // transform on one CPU, and prevent an interrupt handler from sharing
        // temporary registers during this short non-blocking section.
        #[cfg(all(target_arch = "x86_64", dm_aes_ni))]
        let mut fpu_context = FpuContext::new();
        #[cfg(all(target_arch = "x86_64", dm_aes_ni))]
        let _preempt_guard = disable_preempt();
        #[cfg(all(target_arch = "x86_64", dm_aes_ni))]
        let _irq_guard = irq::disable_local();
        #[cfg(all(target_arch = "x86_64", dm_aes_ni))]
        fpu_context.save();

        for (index, sector) in data.chunks_exact_mut(aster_block::SECTOR_SIZE).enumerate() {
            let sector_id = first_sector + index as u64;
            self.transform_sector(sector_id, sector, direction);
        }

        #[cfg(all(target_arch = "x86_64", dm_aes_ni))]
        fpu_context.load();

        Ok(())
    }

    fn transform_sector(&self, sector_id: u64, sector: &mut [u8], direction: XtsDirection) {
        let mut tweak = [0u8; AES_BLOCK_SIZE];
        tweak[..size_of::<u64>()].copy_from_slice(&sector_id.to_le_bytes());
        self.tweak_cipher.encrypt(&mut tweak);

        for block_bytes in sector.chunks_exact_mut(AES_BLOCK_SIZE) {
            for (byte, tweak_byte) in block_bytes.iter_mut().zip(tweak) {
                *byte ^= tweak_byte;
            }

            let block: &mut [u8; AES_BLOCK_SIZE] = block_bytes.try_into().unwrap();
            match direction {
                XtsDirection::Encrypt => self.data_cipher.encrypt(block),
                XtsDirection::Decrypt => self.data_cipher.decrypt(block),
            }

            for (byte, tweak_byte) in block_bytes.iter_mut().zip(tweak) {
                *byte ^= tweak_byte;
            }
            multiply_tweak_by_x(&mut tweak);
        }
    }
}

#[derive(Clone, Copy)]
enum XtsDirection {
    Encrypt,
    Decrypt,
}

fn multiply_tweak_by_x(tweak: &mut [u8; AES_BLOCK_SIZE]) {
    let mut carry = 0u8;
    for byte in tweak.iter_mut() {
        let next_carry = *byte >> 7;
        *byte = (*byte << 1) | carry;
        carry = next_carry;
    }
    if carry != 0 {
        tweak[0] ^= 0x87;
    }
}
