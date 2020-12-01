use super::error::Error;
use super::Result;

pub trait Message {
  fn pipe_id(&self) -> u16;
}

/**
特定のファンクションに対するパイプをオープンするためのメッセージ。
*/
#[derive(Debug)]
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

/**
パイプのクローズを示すメッセージ。`failure` が `false` の場合、この `Close` と対になる `Open` のファンクション
呼び出しは正常に終了し `result` にはその結果が格納されていることを示しています。`failure` が `true` の場合、
ファンクションは何らかの理由で失敗し `result` にはそのエラー状況が可能されていることを示します。
*/
#[derive(Debug)]
pub struct Close {
  /** このメッセージの宛先を示すパイプ ID。 */
  pipe_id: u16,
  /** 処理が失敗した場合 `true`。 */
  failure: bool,
  /** 処理結果を表すバイト配列。処理が異常終了した場合はエラー情報が含まれる。 */
  result: Box<[u8]>,
}

impl Close {
  pub fn new(pipe_id: u16, failure: bool, result: Box<[u8]>) -> Result<Close> {
    verify_pipe_id(pipe_id)?;
    Ok(Close { pipe_id, failure, result })
  }
}

impl Message for Close {
  fn pipe_id(&self) -> u16 {
    self.pipe_id
  }
}

/** Block のペイロードに設定することのできる最大サイズです。0xEFFF (61,439バイト) を表しています。 */
pub const MAX_PAYLOAD_SIZE: usize = 0xEFFF;

/** Block の消失確率に設定することのできる最大値です。0x7F (127) を表しています。 */
pub const MAX_LOSS_RATE: u8 = 0x7F;

#[derive(Debug)]
pub struct Block {
  /** このメッセージの宛先を示すパイプ ID。 */
  pipe_id: u16,

  /**
  転送中にこの Block を消失さても良い確率を示す 0～127 までの値。このフィールドはアプリケーションやネットワークの
  過負荷によってすべての Block を処理できなくなったときに参照されます。値 0 (デフォルト) はどのような状況であっても
  このブロックを消失させてはならないことを表し、127 は 100% の消失が発生しても良いことを示しています。Block が EOF
  を示す場合は必ず 0 にする必要があります。

  この消失確率は一回の消失判定における確率を表しています。つまり、この確率を複数回の消失判定に適用すると、設定者が
  意図した損失確率と異なる結果をもたらすことを意味します。したがって、消失判定を通過した Block の `loss` 値は 0 に
  更新されます。
	*/
  loss: u8,

  /** このブロックが転送するデータ。 */
  payload: Box<[u8]>,

  /** このブロックが EOF を表すかのフラグ。 */
  eof: bool,
}

impl Block {
  pub fn new(pipe_id: u16, loss: u8, payload: Box<[u8]>, eof: bool) -> Result<Block> {
    verify_pipe_id(pipe_id)?;
    if payload.len() > MAX_PAYLOAD_SIZE {
      Err(Error::PayloadTooLarge { length: payload.len(), maximum: MAX_PAYLOAD_SIZE })
    } else if loss > MAX_LOSS_RATE {
      Err(Error::LossRateTooBig { loss: loss as usize, maximum: MAX_LOSS_RATE as usize })
    } else {
      Ok(Block { pipe_id, loss, payload, eof })
    }
  }
}

impl Message for Block {
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
