// SPDX-License-Identifier: MPL-2.0

use aes::{
    Aes256,
    cipher::{BlockDecrypt, BlockEncrypt, NewBlockCipher, generic_array::GenericArray},
};

use crate::DmError;

const AES_BLOCK_SIZE: usize = 16;
const XTS_KEY_SIZE: usize = 64;

/// Encrypts and decrypts sectors with AES-256-XTS.
pub(crate) struct AesXtsCipher {
    data_cipher: Aes256,
    tweak_cipher: Aes256,
}

impl AesXtsCipher {
    /// Creates an AES-256-XTS cipher from a 512-bit key.
    pub(crate) fn new(key: &[u8]) -> Result<Self, DmError> {
        if key.len() != XTS_KEY_SIZE {
            return Err(DmError::InvalidKey);
        }

        let (data_key, tweak_key) = key.split_at(XTS_KEY_SIZE / 2);
        Ok(Self {
            data_cipher: Aes256::new(GenericArray::from_slice(data_key)),
            tweak_cipher: Aes256::new(GenericArray::from_slice(tweak_key)),
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

        for (index, sector) in data.chunks_exact_mut(aster_block::SECTOR_SIZE).enumerate() {
            let sector_id = first_sector
                .checked_add(index as u64)
                .ok_or(DmError::OutOfRange)?;
            self.transform_sector(sector_id, sector, direction);
        }
        Ok(())
    }

    fn transform_sector(&self, sector_id: u64, sector: &mut [u8], direction: XtsDirection) {
        let mut tweak = [0u8; AES_BLOCK_SIZE];
        tweak[..size_of::<u64>()].copy_from_slice(&sector_id.to_le_bytes());
        self.tweak_cipher
            .encrypt_block(GenericArray::from_mut_slice(&mut tweak));

        for block_bytes in sector.chunks_exact_mut(AES_BLOCK_SIZE) {
            for (byte, tweak_byte) in block_bytes.iter_mut().zip(tweak) {
                *byte ^= tweak_byte;
            }

            let block = GenericArray::from_mut_slice(block_bytes);
            match direction {
                XtsDirection::Encrypt => self.data_cipher.encrypt_block(block),
                XtsDirection::Decrypt => self.data_cipher.decrypt_block(block),
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

