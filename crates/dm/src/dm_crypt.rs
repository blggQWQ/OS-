use crate::block_device::BlockDevice;
use crate::crypto::{Aes256XtsCipher, DemoStreamCipher};
use crate::error::{DmError, Result};
use core::str::FromStr;

const DM_SECTOR_SIZE: usize = 512;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DmCryptTable {
    pub cipher: String,
    pub key: Vec<u8>,
    pub iv_offset: u64,
    pub device: String,
    pub offset: u64,
    pub options: Vec<String>,
}

impl FromStr for DmCryptTable {
    type Err = DmError;

    fn from_str(table: &str) -> Result<Self> {
        let fields: Vec<&str> = table.split_whitespace().collect();
        if fields.len() < 5 {
            return Err(DmError::InvalidTable(
                "crypt target needs: cipher key iv_offset device offset".into(),
            ));
        }
        Ok(Self {
            cipher: fields[0].to_string(),
            key: decode_hex(fields[1])?,
            iv_offset: parse_u64("iv_offset", fields[2])?,
            device: fields[3].to_string(),
            offset: parse_u64("offset", fields[4])?,
            options: fields[5..].iter().map(|value| (*value).to_string()).collect(),
        })
    }
}

#[derive(Clone)]
enum CryptMode {
    Demo(DemoStreamCipher),
    Aes256Xts(Aes256XtsCipher),
}

#[derive(Clone)]
pub struct DmCryptDevice<D> {
    lower: D,
    mode: CryptMode,
    iv_offset: u64,
    data_offset_blocks: u64,
    logical_blocks: u64,
}

impl<D: BlockDevice> DmCryptDevice<D> {
    /// Legacy dependency-free demo mode. Use `from_table` for Linux-compatible crypto.
    pub fn new(lower: D, key: &[u8]) -> Result<Self> {
        let logical_blocks = lower.num_blocks();
        Ok(Self {
            lower,
            mode: CryptMode::Demo(DemoStreamCipher::new(key)?),
            iv_offset: 0,
            data_offset_blocks: 0,
            logical_blocks,
        })
    }

    pub fn from_table(lower: D, table: &DmCryptTable) -> Result<Self> {
        if table.cipher != "aes-xts-plain64" {
            return Err(DmError::UnsupportedCipher(table.cipher.clone()));
        }
        let block_size = lower.block_size();
        if block_size % DM_SECTOR_SIZE != 0 {
            return Err(DmError::InvalidBlockSize {
                expected: DM_SECTOR_SIZE,
                actual: block_size,
            });
        }
        let sectors_per_block = (block_size / DM_SECTOR_SIZE) as u64;
        if table.offset % sectors_per_block != 0 {
            return Err(DmError::InvalidTable(format!(
                "data offset {} is not aligned to {} sectors per block",
                table.offset, sectors_per_block
            )));
        }
        let data_offset_blocks = table.offset / sectors_per_block;
        let logical_blocks = lower
            .num_blocks()
            .checked_sub(data_offset_blocks)
            .ok_or_else(|| DmError::InvalidTable("data offset exceeds lower device".into()))?;
        if logical_blocks == 0 {
            return Err(DmError::EmptyDevice);
        }
        Ok(Self {
            lower,
            mode: CryptMode::Aes256Xts(Aes256XtsCipher::new(&table.key)?),
            iv_offset: table.iv_offset,
            data_offset_blocks,
            logical_blocks,
        })
    }

    pub fn lower(&self) -> &D {
        &self.lower
    }

    fn transform(&self, block_id: u64, data: &mut [u8], encrypt: bool) -> Result<()> {
        match &self.mode {
            CryptMode::Demo(cipher) => {
                let input = data.to_vec();
                cipher.apply(block_id, &input, data)
            }
            CryptMode::Aes256Xts(cipher) => {
                let sectors_per_block = data.len() / DM_SECTOR_SIZE;
                let first_sector = self
                    .iv_offset
                    .checked_add(block_id * sectors_per_block as u64)
                    .ok_or_else(|| DmError::InvalidTable("sector number overflow".into()))?;
                for (index, sector_data) in data.chunks_exact_mut(DM_SECTOR_SIZE).enumerate() {
                    let sector = first_sector + index as u64;
                    if encrypt {
                        cipher.encrypt_sector(sector, sector_data)?;
                    } else {
                        cipher.decrypt_sector(sector, sector_data)?;
                    }
                }
                Ok(())
            }
        }
    }
}

impl<D: BlockDevice> BlockDevice for DmCryptDevice<D> {
    fn block_size(&self) -> usize {
        self.lower.block_size()
    }

    fn num_blocks(&self) -> u64 {
        self.logical_blocks
    }

    fn read_block(&self, block_id: u64, buf: &mut [u8]) -> Result<()> {
        validate_io(self, block_id, buf.len())?;
        self.lower
            .read_block(block_id + self.data_offset_blocks, buf)?;
        self.transform(block_id, buf, false)
    }

    fn write_block(&self, block_id: u64, buf: &[u8]) -> Result<()> {
        validate_io(self, block_id, buf.len())?;
        let mut encrypted = vec![0; buf.len()];
        encrypted.copy_from_slice(buf);
        self.transform(block_id, &mut encrypted, true)?;
        self.lower
            .write_block(block_id + self.data_offset_blocks, &encrypted)
    }
}

fn validate_io<D: BlockDevice>(device: &DmCryptDevice<D>, block_id: u64, len: usize) -> Result<()> {
    if len != device.block_size() {
        return Err(DmError::InvalidBlockSize {
            expected: device.block_size(),
            actual: len,
        });
    }
    if block_id >= device.num_blocks() {
        return Err(DmError::InvalidBlockId {
            block_id,
            blocks: device.num_blocks(),
        });
    }
    Ok(())
}

fn parse_u64(name: &str, value: &str) -> Result<u64> {
    value
        .parse()
        .map_err(|_| DmError::InvalidTable(format!("invalid {name}: {value}")))
}

fn decode_hex(value: &str) -> Result<Vec<u8>> {
    if value.len() % 2 != 0 {
        return Err(DmError::InvalidTable("key hex has odd length".into()));
    }
    (0..value.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&value[index..index + 2], 16)
                .map_err(|_| DmError::InvalidTable("key contains non-hex characters".into()))
        })
        .collect()
}
