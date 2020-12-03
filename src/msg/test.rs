use crate::error::Error;
use crate::msg::{Message, MAX_LOSS_RATE, MAX_PAYLOAD_SIZE};
use crate::test::SampleValues;

#[test]
fn test_new_open() {
  let mut sample = SampleValues::new(49087450211597u64);

  // 設定した値と同じ値が参照できる
  let pipe_id = sample.next_u16();
  let function_id = sample.next_u16();
  let priority = sample.next_u8();
  let params = sample.next_bytes(1024);
  match Message::new_open(pipe_id, function_id, priority, params.clone()).unwrap() {
    Message::Open { pipe_id: p1, function_id: p2, priority: p3, params: p4 } => {
      assert_eq!(pipe_id, p1);
      assert_eq!(function_id, p2);
      assert_eq!(priority, p3);
      assert_eq!(params, p4);
    }
    _ => assert!(false),
  }

  // pipe_id に境界値を設定
  assert_eq!(
    Message::new_open(0u16, function_id, priority, params.clone()).unwrap_err(),
    Error::ZeroPipeId
  );
  assert!(Message::new_open(0xFFFFu16, function_id, priority, params.clone()).is_ok());
}

#[test]
fn test_new_close() {
  let mut sample = SampleValues::new(209870247509u64);

  // 設定した値と同じ値が参照できる
  let pipe_id = sample.next_u16();
  let failure = sample.next_bool();
  let result = sample.next_bytes(1024);
  match Message::new_close(pipe_id, failure, result.clone()).unwrap() {
    Message::Close { pipe_id: p1, failure: p2, result: p3 } => {
      assert_eq!(pipe_id, p1);
      assert_eq!(failure, p2);
      assert_eq!(result, p3);
    }
    _ => assert!(false),
  }

  // pipe_id に境界値を設定
  assert_eq!(Message::new_close(0u16, failure, result.clone()).unwrap_err(), Error::ZeroPipeId);
  assert!(Message::new_close(0xFFFFu16, failure, result.clone()).is_ok());
}

#[test]
fn test_new_block() {
  let mut sample = SampleValues::new(572194956990u64);

  // 設定した値と同じ値が参照できる
  let pipe_id = sample.next_u16();
  let loss = sample.next_u8() & 0x7fu8;
  let payload = sample.next_bytes(1024);
  let eof = sample.next_bool();
  match Message::new_block(pipe_id, eof, loss, payload.clone()).unwrap() {
    Message::Block { pipe_id: p1, loss: p2, payload: p3, eof: p4 } => {
      assert_eq!(pipe_id, p1);
      assert_eq!(loss, p2);
      assert_eq!(payload, p3);
      assert_eq!(eof, p4);
    }
    _ => assert!(false),
  }

  // pipe_id に境界値を設定
  assert_eq!(Message::new_block(0u16, eof, loss, payload.clone()).unwrap_err(), Error::ZeroPipeId);
  assert!(Message::new_block(0xFFFFu16, eof, loss, payload.clone()).is_ok());

  assert_eq!(
    Message::new_block(pipe_id, eof, MAX_LOSS_RATE + 1, payload.clone()).unwrap_err(),
    Error::LossRateTooBig { loss: (MAX_LOSS_RATE + 1) as usize, maximum: MAX_LOSS_RATE as usize }
  );
  assert_eq!(
    Message::new_block(pipe_id, eof, loss, sample.next_bytes(MAX_PAYLOAD_SIZE + 1)).unwrap_err(),
    Error::PayloadTooLarge {
      length: (MAX_PAYLOAD_SIZE + 1) as usize,
      maximum: MAX_PAYLOAD_SIZE as usize
    }
  );
}
