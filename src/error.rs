use thiserror::Error as ThisError;

#[derive(ThisError, Debug)]
pub enum Error {
  #[error("the pipe-id can only be zeroed in the Control message")]
  ZeroPipeId,
  // #[error("invalid header (expected {expected:?}, found {found:?})")]
  // InvalidHeader {
  //   expected: String,
  //   found: String,
  // },
}