pub mod dispatcher;

use std::sync::{Arc, RwLock};

use crate::error::Error;
use crate::Result;

/// オープンまたはクローズの状態を持つデータの出力先です。オープン状態のときはデータを `push()` することができますが、
/// クローズ状態で `push()` を行おうとすると失敗します。
pub trait Gate<T> {
  fn set_callback<F: FnMut(GateState) -> ()>(callback: F) -> ();
  fn push(value: T) -> Result<()>;
}

pub enum GateState {
  Writable,
  NotWritable,
  Disposed,
}

pub struct Barrage<T, GATE: Gate<T>> {
  capacity: usize,
  queue: Arc<RwLock<Vec<T>>>,
  gate: GATE,
}

impl<T, GATE: Gate<T>> Barrage<T, GATE> {
  /// 指定された容量を持つメッセージキューを構築します。
  pub fn new(gate: GATE, capacity: usize) -> Barrage<T, GATE> {
    Barrage { capacity, queue: Arc::new(RwLock::new(Vec::new())), gate }
  }

  pub fn capacity(&self) -> usize {
    self.capacity
  }

  pub fn len(&self) -> usize {
    let queue = self.queue.clone();
    let queue = queue.read().unwrap();
    queue.len()
  }

  /// このキューにメッセージを追加します。
  /// 正常に終了した場合、メッセージ追加後のキューのサイズを返します。
  pub fn push(&mut self, msg: T) -> Result<usize> {
    let queue = self.queue.clone();
    let mut queue = queue.write()?;
    if queue.len() == self.capacity {
      Err(Error::MessageQueueOverflow { capacity: self.capacity })
    } else {
      queue.push(msg);
      Ok(queue.len())
    }
  }

  pub fn try_pop(&mut self) -> Result<Option<T>> {
    unimplemented!()
  }
}
