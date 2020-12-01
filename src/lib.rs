pub mod error;
pub mod msg;

type Result<T> = std::result::Result<T, error::Error>;
