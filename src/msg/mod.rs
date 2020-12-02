use uuid::Uuid;

use super::error::Error;
use super::Result;

pub mod codec;

#[cfg(test)]
mod test;

#[derive(Debug)]
pub enum Message {
  /// 特定のファンクションに対するパイプをオープンするためのメッセージ。
  Open {
    /// このメッセージの宛先を示すパイプ ID
    pipe_id: u16,
    /// ファンクションを識別する ID
    function_id: u16,
    /// この Open によって開かれるパイプの同一セッション内での優先度
    priority: u8,
    /// ファンクションの呼び出し時に渡す引数
    params: Vec<u8>,
  },

  /// パイプのクローズを示すメッセージ。`failure` が `false` の場合、この `Close` と対になる `Open` のファンクション
  /// 呼び出しは正常に終了し `result` にはその結果が格納されていることを示しています。`failure` が `true` の場合、
  /// ファンクションは何らかの理由で失敗し `result` にはそのエラー状況が可能されていることを示します。
  Close {
    /** このメッセージの宛先を示すパイプ ID。 */
    pipe_id: u16,
    /** 処理が失敗した場合 `true`。 */
    failure: bool,
    /** 処理結果を表すバイト配列。処理が異常終了した場合はエラー情報が含まれる。 */
    result: Vec<u8>,
  },

  Block {
    /// このメッセージの宛先を示すパイプ ID。
    pipe_id: u16,

    /// 転送中にこの Block を消失さても良い確率を示す 0～127 までの値。このフィールドはアプリケーションやネットワークの
    /// 過負荷によってすべての Block を処理できなくなったときに参照されます。値 0 (デフォルト) はどのような状況であっても
    /// このブロックを消失させてはならないことを表し、127 は 100% の消失が発生しても良いことを示しています。Block が EOF
    /// を示す場合は必ず 0 にする必要があります。
    ///
    /// この消失確率は一回の消失判定における確率を表しています。つまり、この確率を複数回の消失判定に適用すると、設定者が
    /// 意図した損失確率と異なる結果をもたらすことを意味します。したがって、消失判定を通過した Block の `loss` 値は 0 に
    /// 更新されます。
    loss: u8,

    /// このブロックが転送するデータ。
    payload: Vec<u8>,

    /// このブロックが EOF を表すかのフラグ。
    eof: bool,
  },

  Control,
}

impl Message {
  pub fn new_open(pipe_id: u16, function_id: u16, priority: u8, params: Vec<u8>) -> Result<Self> {
    verify_pipe_id(pipe_id)?;
    Ok(Message::Open { pipe_id, function_id, params, priority })
  }

  pub fn new_close(pipe_id: u16, failure: bool, result: Vec<u8>) -> Result<Self> {
    verify_pipe_id(pipe_id)?;
    Ok(Message::Close { pipe_id, failure, result })
  }

  pub fn new_block(pipe_id: u16, loss: u8, payload: Vec<u8>, eof: bool) -> Result<Self> {
    verify_pipe_id(pipe_id)?;
    if payload.len() > MAX_PAYLOAD_SIZE {
      Err(Error::PayloadTooLarge { length: payload.len(), maximum: MAX_PAYLOAD_SIZE })
    } else if loss > MAX_LOSS_RATE {
      Err(Error::LossRateTooBig { loss: loss as usize, maximum: MAX_LOSS_RATE as usize })
    } else {
      Ok(Message::Block { pipe_id, loss, payload, eof })
    }
  }
}

/// Block のペイロードに設定することのできる最大サイズです。0xEFFF (61,439バイト) を表しています。
pub const MAX_PAYLOAD_SIZE: usize = 0xEFFF;

/// Block の消失確率に設定することのできる最大値です。0x7F (127) を表しています。
pub const MAX_LOSS_RATE: u8 = 0x7F;

pub enum Control {
  SystemConfig {
    /** プロトコルのバージョンを示す 2 バイト整数値。上位バイトから [major][minor] の順を持つ。 */
    version: u16,
    /** ノード ID。TLS を使用している場合は証明書の CNAME と照合する必要がある。 */
    node_id: Uuid,
    /** セッション ID。クライアントからの接続後の Sync に対するサーバ応答でのみ有効な値を持つ。それ以外の場合、
    送信者は Zero を送らなければならず、受信者は無視しなければならない。 */
    session_id: Uuid,
    /** UTC ミリ秒で表現したローカル実行環境の現在時刻。 */
    utc_time: u64,
    /** サーバからクライアントへセッションの死活監視を行うための ping 間隔 (秒)。 */
    ping_interval: u32,
    /** セッションタイムアウトまでの間隔 (秒)。 */
    session_timeout: u32,
  }
}

fn verify_pipe_id(pipe_id: u16) -> Result<()> {
  if pipe_id == 0 {
    Err(Error::ZeroPipeId)
  } else {
    Ok(())
  }
}
