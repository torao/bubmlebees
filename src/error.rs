use thiserror::Error as ThisError;

#[derive(ThisError, Debug, PartialEq, Eq)]
pub enum Error {
  #[error("should receive more data to restore the entire message")]
  BufferUnsatisfied,

  #[error("the pipe-id can only be zeroed in the Control message")]
  ZeroPipeId,
  #[error("too large payload: {length:?}, max={maximum:?}")]
  PayloadTooLarge { length: usize, maximum: usize },
  #[error("too big loss rate: {loss:?}, max={maximum:?}")]
  LossRateTooBig { loss: usize, maximum: usize },

  #[error("illegal boolean representation: {value:#04X}")]
  IllegalBooleanRepresentation { value: u8 },
  #[error("illegal Control type: {value:#04X}")]
  IllegalControlType { value: u8 },
  #[error("underlying I/O layer error: {message}")]
  Io {
    kind: std::io::ErrorKind,
    message: String, // TODO use of unstable library feature 'backtrace'
                     // TODO see issue #53487 <https://github.com/rust-lang/rust/issues/53487> for more information
                     // #[source]
                     // source: std::io::Error
                     // #[from]
                     // source: std::io::Error,
                     // backtrace: std::backtrace::Backtrace
  },
}

impl From<std::io::Error> for Error {
  fn from(err: std::io::Error) -> Error {
    if err.kind() == std::io::ErrorKind::UnexpectedEof {
      Error::BufferUnsatisfied
    } else {
      Error::Io { kind: err.kind(), message: err.to_string() }
    }
  }
}
