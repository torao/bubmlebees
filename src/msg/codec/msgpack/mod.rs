use std::io::{Read, Write};

use rmp as msgpack;
use rmp::decode::ValueReadError;
use rmp::encode::ValueWriteError;

use crate::error::Error;
use crate::msg::Message;
use crate::Result;

pub fn encode<W: Write>(buf: &mut W, msg: &Message) -> Result<()> {
  match msg {
    Message::Open { pipe_id, function_id, priority, params } => {
      write_u16(buf, *pipe_id)?;
      write_u16(buf, *function_id)?;
      write_u8(buf, *priority)?;
      write_bin(buf, params)?;
    }
    _ => unimplemented!() // TODO
  }
  Ok(())
}

pub fn decode_open<R: Read>(buf: &mut R) -> Result<Message> {
  Ok(Message::Open {
    pipe_id: read_u16(buf)?,
    function_id: read_u16(buf)?,
    priority: read_u8(buf)?,
    params: read_bin(buf)?,
  })
}

#[inline]
fn write_u8<W: Write>(buf: &mut W, value: u8) -> Result<()> {
  msgpack::encode::write_u8(buf, value).map_err(Error::from)
}

#[inline]
fn read_u8<R: Read>(buf: &mut R) -> Result<u8> {
  msgpack::decode::read_u8(buf).map_err(Error::from)
}

#[inline]
fn write_u16<W: Write>(buf: &mut W, value: u16) -> Result<()> {
  msgpack::encode::write_u16(buf, value).map_err(Error::from)
}

#[inline]
fn read_u16<R: Read>(buf: &mut R) -> Result<u16> {
  msgpack::decode::read_u16(buf).map_err(Error::from)
}

#[inline]
fn write_bin<W: Write>(buf: &mut W, value: &[u8]) -> Result<()> {
  msgpack::encode::write_bin_len(buf, value.len() as u32)?;
  buf.write_all(value).map_err(Error::from)
}

#[inline]
fn read_bin<R: Read>(buf: &mut R) -> Result<Vec<u8>> {
  let expected_length = msgpack::decode::read_bin_len(buf)?;
  let mut buffer = Vec::<u8>::with_capacity(expected_length as usize);
  let actual_read_length = buf.read(&mut buffer)?;
  if (actual_read_length as u32) < expected_length {
    Err(Error::BufferUnsatisfied)
  } else {
    Ok(buffer)
  }
}

impl From<ValueWriteError> for Error {
  fn from(err: ValueWriteError) -> Error {
    Error::Io { message: err.to_string() }
  }
}

impl From<ValueReadError> for Error {
  fn from(err: ValueReadError) -> Error {
    Error::Io { message: err.to_string() }
  }
}