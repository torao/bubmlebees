use thiserror::Error as ThisError;

#[derive(ThisError, Debug)]
pub enum Error {
  #[error("the pipe-id can only be zeroed in the Control message")]
  ZeroPipeId,
  #[error("too large payload: {length:?}, max={maximum:?}")]
  PayloadTooLarge { length: usize, maximum: usize },
  #[error("too big loss rate: {loss:?}, max={maximum:?}")]
  LossRateTooBig { loss: usize, maximum: usize },
}