pub mod error;
pub mod msg;

#[cfg(test)]
mod test;

type Result<T> = std::result::Result<T, error::Error>;
