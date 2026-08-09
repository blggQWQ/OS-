// fmt是格式化模块，用来让自定义类型可以被格式化输出为字符串
use core::fmt;

// 定义 Result 类型，用来表示操作的结果
pub type Result<T> = core::result::Result<T, DmError>;

#[derive(Debug, Clone, PartialEq, Eq)]
// derive(Debug)   - 可以用 println!("{:?}", error) 打印调试信息
// derive(Clone)   - 可以 .clone() 复制
// derive(PartialEq) - 可以用 == 比较两个错误是否相同
// derive(Eq)      - 完全相等比较（PartialEq 的加强版）

// enum 是枚举，列出所有可能的错误类型
pub enum DmError {
    // 块大小不对：期望 expected 字节，实际给了 actual 字节
    InvalidBlockSize { expected: usize, actual: usize },
    // 块编号不对：给了 block_id，但设备只有 blocks 个块
    InvalidBlockId { block_id: u64, blocks: u64 },
    // 图像大小不对：期望 image_size 字节，实际给了 actual 字节
    InvalidImageSize { image_size: u64, block_size: usize },
    // 设备容量溢出：blocks 个块，每块 block_size 字节，超过平台限制
    CapacityOverflow { blocks: u64, block_size: usize },
    // 错误的加密密钥
    InvalidKey,
    // 设备映射表（table）格式不对：message 是错误信息
    InvalidTable(String),
    // 不支持的加密算法：cipher 是指定的加密算法名称
    UnsupportedCipher(String),
    // 数据完整性校验失败：block_id 是出错的块编号
    // dm-verity 机制会对每个块进行哈希校验，如果校验失败就会报这个错误
    IntegrityViolation { block_id: u64 },
    // 设备是只读的，不能写入数据
    ReadOnlyDevice,
    // 设备为空，一页都没有
    EmptyDevice,
    // 输入输出错误（读写文件失败），message 是错误信息
    Io(String),
}

// 实现 Display trait，让 DmError 可以被转成人类可读的字符串
impl fmt::Display for DmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBlockSize { expected, actual } => {
                write!(f, "invalid block size: expected {expected}, got {actual}")
            }
            Self::InvalidBlockId { block_id, blocks } => {
                write!(f, "invalid block id {block_id}; device has {blocks} blocks")
            }
            Self::InvalidImageSize {
                image_size,
                block_size,
            } => write!(
                f,
                "invalid image size {image_size}; not divisible by block size {block_size}"
            ),
            Self::CapacityOverflow { blocks, block_size } => write!(
                f,
                "device capacity overflows the platform: {blocks} blocks of {block_size} bytes"
            ),
            Self::InvalidKey => write!(f, "invalid encryption key"),
            Self::InvalidTable(message) => write!(f, "invalid device-mapper table: {message}"),
            Self::UnsupportedCipher(cipher) => write!(f, "unsupported cipher: {cipher}"),
            Self::IntegrityViolation { block_id } => {
                write!(f, "integrity verification failed for block {block_id}")
            }
            Self::ReadOnlyDevice => write!(f, "device is read-only"),
            Self::EmptyDevice => write!(f, "device has no blocks"),
            Self::Io(message) => write!(f, "I/O error: {message}"),
        }
    }
}

// 实现 std::error::Error trait，让 DmError 成为 Rust 标准的错误类型
// 这样就可以用 ? 操作符、和标准库的错误处理体系配合使用
impl std::error::Error for DmError {}

// 实现 From<std::io::Error>，让标准库的 IO 错误可以自动转成 DmError
// 这样在代码里就可以用 ? 操作符把 std::io::Error 自动转成 DmError
impl From<std::io::Error> for DmError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value.to_string())
    }
}