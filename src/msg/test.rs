use std::io::Cursor;
use std::time::{SystemTime, UNIX_EPOCH};

use uuid::Uuid;

use crate::error::Error;
use crate::msg::{Block, Close, Control, MAX_LOSS_RATE, MAX_PAYLOAD_SIZE, Open};
use crate::test::SampleValues;

#[test]
fn test_open_new() {
  let mut sample = SampleValues::new(49087450211597u64);

  // 設定した値と同じ値が参照できる
  let pipe_id = sample.next_u16();
  let function_id = sample.next_u16();
  let priority = sample.next_u8();
  let params = sample.next_bytes(1024);
  let open = Open::new(pipe_id, function_id, priority, params.clone()).unwrap();
  assert_eq!(pipe_id, open.pipe_id);
  assert_eq!(function_id, open.function_id);
  assert_eq!(priority, open.priority);
  assert_eq!(params, open.params);

  // pipe_id に境界値を設定
  assert_eq!(
    Open::new(0u16, function_id, priority, params.clone()).unwrap_err(),
    Error::ZeroPipeId
  );
  assert!(Open::new(0xFFFFu16, function_id, priority, params.clone()).is_ok());
}

#[test]
fn test_open_read_write() {
  // バイナリ表現が想定と一致しているか
  let mut buf = Vec::new();
  let open = Open::new(1u16, 2u16, 3u8, Vec::from([4u8, 5u8])).unwrap();
  open.write_to(&mut buf).unwrap();
  assert_eq!(&[0x01u8, 0x00, 0x02, 0x00, 0x03, 0x02, 0x00, 0x04, 0x05][..], buf);

  // 復元したメッセージが元の値と一致しているか
  let restored = Open::read_from(&mut Cursor::new(&buf[..])).unwrap();
  assert_eq!(open, restored);

  // 未完成のバッファを検出できるか
  for i in 0..(buf.len() - 1) {
    assert_eq!(
      Error::BufferUnsatisfied,
      Open::read_from(&mut Cursor::new(&buf[0..i])).unwrap_err()
    );
  }
}

#[test]
fn test_close_new() {
  let mut sample = SampleValues::new(209870247509u64);

  // 設定した値と同じ値が参照できる
  let pipe_id = sample.next_u16();
  let failure = sample.next_bool();
  let result = sample.next_bytes(1024);
  let close = Close::new(pipe_id, failure, result.clone()).unwrap();
  assert_eq!(pipe_id, close.pipe_id);
  assert_eq!(failure, close.failure);
  assert_eq!(result, close.result);

  // pipe_id に境界値を設定
  assert_eq!(Close::new(0u16, failure, result.clone()).unwrap_err(), Error::ZeroPipeId);
  assert!(Close::new(0xFFFFu16, failure, result.clone()).is_ok());
}

#[test]
fn test_close_read_write() {
  // バイナリ表現が想定と一致しているか
  let mut buf = Vec::new();
  let close = Close::new(1u16, true, Vec::from([2u8, 3])).unwrap();
  close.write_to(&mut buf).unwrap();
  assert_eq!(&[0x01u8, 0x00, 0x01, 0x02, 0x00, 0x02, 0x03][..], buf);

  // 復元したメッセージが元の値と一致しているか
  let restored = Close::read_from(&mut Cursor::new(&buf[..])).unwrap();
  assert_eq!(close, restored);

  // 未完成のバッファを検出できるか
  for i in 0..(buf.len() - 1) {
    assert_eq!(
      Error::BufferUnsatisfied,
      Close::read_from(&mut Cursor::new(&buf[0..i])).unwrap_err()
    );
  }
}

#[test]
fn test_block_new() {
  let mut sample = SampleValues::new(572194956990u64);

  // 設定した値と同じ値が参照できる
  let pipe_id = sample.next_u16();
  let loss = sample.next_u8() & 0x7fu8;
  let payload = sample.next_bytes(1024);
  let eof = sample.next_bool();
  let block = Block::new(pipe_id, eof, loss, payload.clone()).unwrap();
  assert_eq!(pipe_id, block.pipe_id);
  assert_eq!(loss, block.loss);
  assert_eq!(payload, block.payload);
  assert_eq!(eof, block.eof);

  // pipe_id に境界値を設定
  assert_eq!(Block::new(0u16, eof, loss, payload.clone()).unwrap_err(), Error::ZeroPipeId);
  assert!(Block::new(0xFFFFu16, eof, loss, payload.clone()).is_ok());

  // loss に上限以上の値を設定
  assert_eq!(
    Block::new(pipe_id, eof, MAX_LOSS_RATE + 1, payload.clone()).unwrap_err(),
    Error::LossRateTooBig { loss: (MAX_LOSS_RATE + 1) as usize, maximum: MAX_LOSS_RATE as usize }
  );

  // payload に上限以上の長さを設定
  assert_eq!(
    Block::new(pipe_id, eof, loss, sample.next_bytes(MAX_PAYLOAD_SIZE + 1)).unwrap_err(),
    Error::PayloadTooLarge {
      length: (MAX_PAYLOAD_SIZE + 1) as usize,
      maximum: MAX_PAYLOAD_SIZE as usize,
    }
  );
}

#[test]
fn test_block_read_write() {
  // バイナリ表現が想定と一致しているか
  let mut buf = Vec::new();
  let block = Block::new(1u16, true, 2u8, Vec::from([3u8, 4])).unwrap();
  block.write_to(&mut buf).unwrap();
  assert_eq!(&[0x01u8, 0x00, (1 << 7) | 0x02, 0x02, 0x00, 0x03, 0x04][..], buf);

  // 復元したメッセージが元の値と一致しているか
  let restored = Block::read_from(&mut Cursor::new(&buf[..])).unwrap();
  assert_eq!(block, restored);

  // 未完成のバッファを検出できるか
  for i in 0..(buf.len() - 1) {
    assert_eq!(
      Error::BufferUnsatisfied,
      Block::read_from(&mut Cursor::new(&buf[0..i])).unwrap_err()
    );
  }
}

#[test]
fn test_control_new_system_config() {
  let mut sample = SampleValues::new(48907095721u64);

  // 設定した値と同じ値が参照できる
  let version = sample.next_u16();
  let node_id = sample.next_uuid();
  let session_id = sample.next_uuid();
  let utc_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros() as u64;
  let ping_interval = sample.next_u32();
  let session_timeout = sample.next_u32();
  if let Control::SystemConfig {
    version: p1,
    node_id: p2,
    session_id: p3,
    utc_time: p4,
    ping_interval: p5,
    session_timeout: p6,
  } = Control::new_system_config(
    version,
    node_id,
    session_id,
    utc_time,
    ping_interval,
    session_timeout,
  )
    .unwrap()
  {
    assert_eq!(version, p1);
    assert_eq!(node_id, p2);
    assert_eq!(session_id, p3);
    assert_eq!(utc_time, p4);
    assert_eq!(ping_interval, p5);
    assert_eq!(session_timeout, p6);
  } else {
    assert!(false);
  }
}

#[test]
fn test_control_system_config_read_write() {
  // バイナリ表現が想定と一致しているか
  let mut buf = Vec::new();
  let sys_config = Control::new_system_config(
    1u16,
    Uuid::from_u128(2u128),
    Uuid::from_u128(3u128),
    4u64,
    5u32,
    6u32,
  )
    .unwrap();
  sys_config.write_to(&mut buf).unwrap();
  assert_eq!(
    &[
      'Q' as u8, 0x01, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
      0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
      0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x05,
      0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00
    ][..],
    buf
  );

  // 復元したメッセージが元の値と一致しているか
  let restored = Control::read_from(&mut Cursor::new(&buf[..])).unwrap();
  assert_eq!(sys_config, restored);

  // 未完成のバッファを検出できるか
  for i in 0..(buf.len() - 1) {
    assert_eq!(
      Error::BufferUnsatisfied,
      Control::read_from(&mut Cursor::new(&buf[0..i])).unwrap_err()
    );
  }
}

#[test]
fn test_control_new_ping() {

  // 設定した値と同じ値が参照できる
  let utc_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros() as u64;
  if let Control::Ping { utc_time: p1 } = Control::new_ping(utc_time).unwrap() {
    assert_eq!(utc_time, p1);
  } else {
    assert!(false);
  }
}

#[test]
fn test_control_ping_read_write() {
  // バイナリ表現が想定と一致しているか
  let mut buf = Vec::new();
  let ping = Control::new_ping(1u64).unwrap();
  ping.write_to(&mut buf).unwrap();
  assert_eq!(&['P' as u8, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..], buf);

  // 復元したメッセージが元の値と一致しているか
  let restored = Control::read_from(&mut Cursor::new(&buf[..])).unwrap();
  assert_eq!(ping, restored);

  // 未完成のバッファを検出できるか
  for i in 0..(buf.len() - 1) {
    assert_eq!(
      Error::BufferUnsatisfied,
      Control::read_from(&mut Cursor::new(&buf[0..i])).unwrap_err()
    );
  }
}
