use core::fmt;

pub type Result<T> = core::result::Result<T, DmError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DmError {
    InvalidBlockSize { expected: usize, actual: usize },
    InvalidBlockId { block_id: u64, blocks: u64 },
    InvalidImageSize { image_size: u64, block_size: usize },
    InvalidKey,
    InvalidTable(String),
    UnsupportedCipher(String),
    IntegrityViolation { block_id: u64 },
    ReadOnlyDevice,
    EmptyDevice,
    Io(String),
}

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

impl std::error::Error for DmError {}

impl From<std::io::Error> for DmError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value.to_string())
    }
}
