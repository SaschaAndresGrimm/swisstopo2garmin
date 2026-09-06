use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("http error for {url}: {source}")]
    Http {
        url: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("unexpected http status {status} for {url}")]
    Status { url: String, status: u16 },

    #[error("server does not support range requests for {url}, which this download needs")]
    NoRangeSupport { url: String },

    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("{algo} checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch {
        algo: String,
        expected: String,
        actual: String,
    },

    #[error("size mismatch: expected {expected} bytes, got {actual}")]
    SizeMismatch { expected: u64, actual: u64 },

    #[error("unsupported multihash code 0x{code:02x}")]
    UnsupportedMultihash { code: u8 },

    #[error("malformed multihash: {0}")]
    MalformedMultihash(String),

    #[error("not found in STAC catalog: {0}")]
    NotFound(String),

    #[error("zip error: {0}")]
    Zip(String),

    #[error("decompression failed: {0}")]
    Inflate(String),

    #[error("cancelled")]
    Cancelled,

    #[error("insufficient disk space at {path}: need {need} bytes, {available} available")]
    InsufficientSpace {
        path: PathBuf,
        need: u64,
        available: u64,
    },
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::Io {
            path: path.into(),
            source,
        }
    }
}
