use super::error::Error;
use super::Result;

pub trait Message {
  fn pipe_id(&self) -> u16;
}

/** 特定のファンクションに対するパイプをオープンするためのメッセージ */
#[Debug]
pub struct Open {
  /** このメッセージの宛先を示すパイプ ID */
  pipe_id: u16,
  /** ファンクションを識別する ID */
  function_id: u16,
  /** ファンクションの呼び出し時に渡す引数 */
  params: Box<[u8]>,
  /** この Open によって開かれるパイプの同一セッション内での優先度 */
  priority: u8,
}

impl Open {
  pub fn new(pipe_id: u16, function_id: u16, params: Box<[u8]>, priority: u8) -> Result<Open> {
    verify_pipe_id(pipe_id)?;
    Ok(Open { pipe_id, function_id, params, priority })
  }
}

impl Message for Open {
  fn pipe_id(&self) -> u16 {
    self.pipe_id
  }
}

/** パイプのクローズを示すメッセージです。
 * {@code result} もしくは {@code abort} のどちらかが有効な値を持ちます。
 */
pub struct Close {
  /** このメッセージの宛先を示すパイプ ID。 */
  pipe_id: u16,
  /** 処理が異常終了した場合 True。 */
  aborted: bool,
  /** 処理結果を表すバイト配列。処理が異常終了した場合はエラー情報が含まれる。 */
  result: Box<[b8]>,
}

impl Close {
  pub fn new(pipe_id: u16, aborted: bool, result: Box<[u8]>) -> Result<Close> {
    verify_pipe_id(pipe_id)?;
    Ok(Close { pipe_id, aborted, result })
  }
}

impl Message for Close {
  fn pipe_id(&self) -> u16 {
    self.pipe_id
  }
}

fn verify_pipe_id(pipe_id: u16) -> Result<()> {
  if pipe_id == 0 {
    Err(Error::ZeroPipeId)
  } else {
    Ok(())
  }
}
