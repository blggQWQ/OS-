// 从 crate::error 引入 DmError 和 Result 类型
use crate::error::{DmError, Result};

// 从 crate::sha256 引入 sha256 函数，SHA-256可以把任意数据变成固定32字节的“指纹”
use crate::sha256::sha256;

// 从 aes 库引入GenericArray, BlockDecrypt, BlockEncrypt, KeyInit
// GenericArray固定长度的数组，AES 库规定用它传数据
// BlockDecrypt, 解密一块数据的能力
// BlockEncrypt, 加密一块数据的能力
// KeyInit，初始化密钥的能力
use aes::cipher::{generic_array::GenericArray, BlockDecrypt, BlockEncrypt, KeyInit};

// 从 aes 库引入 Aes256，AES-256加密算法的实现,密钥长度为256字节
use aes::Aes256;

// 定义一个结构体 DemoStreamCipher，用于实现流式加密，可以提供复制，打印调试信息的能力
#[derive(Clone, Debug)]
pub struct DemoStreamCipher {
    key: Vec<u8>,
}

// 定义一个结构体 Aes256XtsCipher，用于实现AES-256-XTS加密算法，可以提供复制的能力
#[derive(Clone)]
pub struct Aes256XtsCipher {
    data: Aes256,
    tweak: Aes256,
}

// 实现 Aes256XtsCipher 的方法
impl Aes256XtsCipher {
    // 定义一个常量 KEY_BYTES，用于表示密钥的长度，64字节
    pub const KEY_BYTES: usize = 64;

    // 定义一个常量 SECTOR_BYTES，用于表示每个扇区的字节数，512字节
    pub const SECTOR_BYTES: usize = 512;

    // new() 用密钥创建加密器
    pub fn new(key: &[u8]) -> Result<Self> {
        // 检查密钥长度是否正确，如果不正确就返回 DmError::InvalidKey 错误
        if key.len() != Self::KEY_BYTES {
            return Err(DmError::InvalidKey);
        }

        // 创建 Aes256XtsCipher 实例，使用前32字节初始化 data，后32字节初始化 tweak
        Ok(Self {
            data: Aes256::new(GenericArray::from_slice(&key[..32])),
            tweak: Aes256::new(GenericArray::from_slice(&key[32..])),
        })
    }

    // encrypt_sector() 用于加密一个扇区的数据
    pub fn encrypt_sector(&self, sector: u64, data: &mut [u8]) -> Result<()> {
        self.crypt_sector(sector, data, true)
    }

    // decrypt_sector() 用于解密一个扇区的数据
    pub fn decrypt_sector(&self, sector: u64, data: &mut [u8]) -> Result<()> {
        self.crypt_sector(sector, data, false)
    }

    // crypt_sector() 用于加密或解密一个扇区的数据
    // encrypt: true 表示加密，false 表示解密
    // 扇区号+原始数据+加密/解密标志
    fn crypt_sector(&self, sector: u64, data: &mut [u8], encrypt: bool) -> Result<()> {
        // 检查数据长度是否为0或者不是16的倍数，如果不符合就返回 DmError::InvalidBlockSize 错误
        if data.is_empty() || data.len() % 16 != 0 {
            return Err(DmError::InvalidBlockSize {
                expected: Self::SECTOR_BYTES,
                actual: data.len(),
            });
        }

        // 创建一个16字节的全0tweak数组，用于存储扇区号的加密结果
        let mut tweak = [0u8; 16];

        // 把扇区号写进前8字节
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

// 实现 DemoStreamCipher 的方法
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
