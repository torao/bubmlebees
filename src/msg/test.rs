use crate::msg::Message;
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
    _ => assert!(false)
  }

  // pipe_id に様々な値を設定
  assert!(Message::new_open(0u16, function_id, priority, params.clone()).is_err());
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
    _ => assert!(false)
  }

  // pipe_id に様々な値を設定
  assert!(Message::new_close(0u16, failure, result.clone()).is_err());
  assert!(Message::new_close(0xFFFFu16, failure, result.clone()).is_ok());
}
