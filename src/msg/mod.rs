use std::io::{Read, Write};

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use uuid::Uuid;

use super::error::Error;
use super::Result;

#[cfg(test)]
mod test;

/// Block のペイロードに設定することのできる最大サイズです。0xEFFF (61,439バイト) を表しています。
pub const MAX_PAYLOAD_SIZE: usize = 0xEFFF;

/// Block の消失確率に設定することのできる最大値です。0x7F (127) を表しています。
pub const MAX_LOSS_RATE: u8 = 0x7F;

/// シリアライズした 1 メッセージの最大バイナリ長です。IPv4 のデータ部最大長である 65,507 を表します。
pub const MAX_MESSAGE_SIZE: usize = 65507;

/// 特定のファンクションに対するパイプをオープンするためのメッセージ。
#[derive(Debug, PartialEq)]
pub struct Open {
  /// このメッセージの宛先を示すパイプ ID
  pipe_id: u16,
  /// ファンクションを識別する ID
  function_id: u16,
  /// この Open によって開かれるパイプの同一セッション内での優先度
  priority: u8,
  /// ファンクションの呼び出し時に渡す引数
  params: Vec<u8>,
}

impl Open {
  pub fn new(pipe_id: u16, function_id: u16, priority: u8, params: Vec<u8>) -> Result<Self> {
    verify_pipe_id(pipe_id)?;
    Ok(Open { pipe_id, function_id, params, priority })
  }

  pub fn write_to<W: Write>(&self, buf: &mut W) -> Result<()> {
    write_u16(buf, self.pipe_id)?;
    write_u16(buf, self.function_id)?;
    write_u8(buf, self.priority)?;
    write_bin(buf, &self.params)?;
    Ok(())
  }

  pub fn read_from<R: Read>(buf: &mut R) -> Result<Open> {
    Ok(Open {
      pipe_id: read_u16(buf)?,
      function_id: read_u16(buf)?,
      priority: read_u8(buf)?,
      params: read_bin(buf)?,
    })
  }
}

/// パイプのクローズを示すメッセージ。`failure` が `false` の場合、この `Close` と対になる `Open` のファンクション
/// 呼び出しは正常に終了し `result` にはその結果が格納されていることを示しています。`failure` が `true` の場合、
/// ファンクションは何らかの理由で失敗し `result` にはそのエラー状況が可能されていることを示します。
#[derive(Debug, PartialEq)]
pub struct Close {
  /** このメッセージの宛先を示すパイプ ID。 */
  pipe_id: u16,
  /** 処理が失敗した場合 `true`。 */
  failure: bool,
  /** 処理結果を表すバイト配列。処理が異常終了した場合はエラー情報が含まれる。 */
  result: Vec<u8>,
}

impl Close {
  pub fn new(pipe_id: u16, failure: bool, result: Vec<u8>) -> Result<Self> {
    verify_pipe_id(pipe_id)?;
    Ok(Close { pipe_id, failure, result })
  }

  pub fn write_to<W: Write>(&self, buf: &mut W) -> Result<()> {
    let bit_field: u8 = if self.failure { 1 << 0 } else { 0 };
    write_u16(buf, self.pipe_id)?;
    write_u8(buf, bit_field)?;
    write_bin(buf, &self.result)?;
    Ok(())
  }

  pub fn read_from<R: Read>(buf: &mut R) -> Result<Close> {
    let pipe_id = read_u16(buf)?;
    let bit_field = read_u8(buf)?;
    let result = read_bin(buf)?;
    Ok(Close { pipe_id, failure: (bit_field & 0x01) != 0, result })
  }
}

#[derive(Debug, PartialEq)]
pub struct Block {
  /// このメッセージの宛先を示すパイプ ID。
  pipe_id: u16,

  /// このブロックが EOF を表すかのフラグ。
  eof: bool,

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
}

impl Block {
  pub fn new(pipe_id: u16, eof: bool, loss: u8, payload: Vec<u8>) -> Result<Self> {
    verify_pipe_id(pipe_id)?;
    if payload.len() > MAX_PAYLOAD_SIZE {
      Err(Error::PayloadTooLarge { length: payload.len(), maximum: MAX_PAYLOAD_SIZE })
    } else if loss > MAX_LOSS_RATE {
      Err(Error::LossRateTooBig { loss: loss as usize, maximum: MAX_LOSS_RATE as usize })
    } else {
      Ok(Block { pipe_id, eof, loss, payload })
    }
  }

  pub fn write_to<W: Write>(&self, buf: &mut W) -> Result<()> {
    debug_assert!(self.loss & (1 << 7) == 0u8);
    let bit_field: u8 = self.loss | if self.eof { 1 << 7 } else { 0 };
    write_u16(buf, self.pipe_id)?;
    write_u8(buf, bit_field)?;
    write_bin(buf, &self.payload)?;
    Ok(())
  }

  pub fn read_from<R: Read>(buf: &mut R) -> Result<Block> {
    let pipe_id = read_u16(buf)?;
    let bit_field = read_u8(buf)?;
    let payload = read_bin(buf)?;
    Ok(Block { pipe_id, eof: bit_field & (1 << 7) != 0, loss: bit_field & 0x7Fu8, payload })
  }
}

#[derive(Debug, PartialEq)]
pub enum Control {
  SystemConfig {
    /// プロトコルのバージョンを示す 2 バイト整数値。上位バイトから [major][minor] の順を持つ。
    version: u16,
    /// ノード ID。TLS を使用している場合は証明書の CNAME と照合する必要がある。
    node_id: Uuid,
    /// セッション ID。クライアントからの接続後の Sync に対するサーバ応答でのみ有効な値を持つ。それ以外の場合、
    /// 送信者は Zero を送らなければならず、受信者は無視しなければならない。
    session_id: Uuid,
    /// UTC ミリ秒で表現したローカル実行環境の現在時刻。
    utc_time: u64,
    /// サーバからクライアントへセッションの死活監視を行うための ping 間隔 (秒)。
    ping_interval: u32,
    /// セッションタイムアウトまでの間隔 (秒)。
    session_timeout: u32,
  },
  Ping {
    /** UTC ミリ秒で表現したローカル実行環境の現在時刻。 */
    utc_time: u64,
  },
}

/// System Config コントロールメッセージの識別子。
const ID_CTRL_SYSCONFIG: u8 = 'Q' as u8;

/// Ping コントロールメッセージの識別子。
const ID_CTRL_PING: u8 = 'P' as u8;

impl Control {
  /// System Config コントロールメッセージを構築します。
  pub fn new_system_config(
    version: u16,
    node_id: Uuid,
    session_id: Uuid,
    utc_time: u64,
    ping_interval: u32,
    session_timeout: u32,
  ) -> Result<Control> {
    Ok(Control::SystemConfig {
      version,
      node_id,
      session_id,
      utc_time,
      ping_interval,
      session_timeout,
    })
  }

  /// Ping コントロールメッセージを構築します。
  pub fn new_ping(utc_time: u64) -> Result<Control> {
    Ok(Control::Ping { utc_time })
  }

  pub fn write_to<W: Write>(&self, buf: &mut W) -> Result<()> {
    match self {
      Control::SystemConfig {
        version,
        node_id,
        session_id,
        utc_time,
        ping_interval,
        session_timeout,
      } => {
        write_u8(buf, ID_CTRL_SYSCONFIG)?;
        write_u16(buf, *version)?;
        write_u128(buf, node_id.as_u128())?;
        write_u128(buf, session_id.as_u128())?;
        write_u64(buf, *utc_time)?;
        write_u32(buf, *ping_interval)?;
        write_u32(buf, *session_timeout)?;
      }
      Control::Ping { utc_time } => {
        write_u8(buf, ID_CTRL_PING)?;
        write_u64(buf, *utc_time)?;
      }
    }
    Ok(())
  }

  pub fn read_from<R: Read>(buf: &mut R) -> Result<Control> {
    match read_u8(buf)? {
      ID_CTRL_SYSCONFIG => Ok(Control::SystemConfig {
        version: read_u16(buf)?,
        node_id: Uuid::from_u128(read_u128(buf)?),
        session_id: Uuid::from_u128(read_u128(buf)?),
        utc_time: read_u64(buf)?,
        ping_interval: read_u32(buf)?,
        session_timeout: read_u32(buf)?,
      }),
      ID_CTRL_PING => Ok(Control::Ping { utc_time: read_u64(buf)? }),
      unexpected => Err(Error::IllegalControlType { value: unexpected }),
    }
  }
}

fn verify_pipe_id(pipe_id: u16) -> Result<()> {
  if pipe_id == 0 {
    Err(Error::ZeroPipeId)
  } else {
    Ok(())
  }
}

#[inline]
fn write_u8<W: Write>(buf: &mut W, value: u8) -> Result<()> {
  buf.write_u8(value).map_err(Error::from)
}

#[inline]
fn read_u8<R: Read>(buf: &mut R) -> Result<u8> {
  buf.read_u8().map_err(Error::from)
}

#[inline]
fn write_u16<W: Write>(buf: &mut W, value: u16) -> Result<()> {
  buf.write_u16::<LittleEndian>(value).map_err(Error::from)
}

#[inline]
fn read_u16<R: Read>(buf: &mut R) -> Result<u16> {
  buf.read_u16::<LittleEndian>().map_err(Error::from)
}

#[inline]
fn write_u32<W: Write>(buf: &mut W, value: u32) -> Result<()> {
  buf.write_u32::<LittleEndian>(value).map_err(Error::from)
}

#[inline]
fn read_u32<R: Read>(buf: &mut R) -> Result<u32> {
  buf.read_u32::<LittleEndian>().map_err(Error::from)
}

#[inline]
fn write_u64<W: Write>(buf: &mut W, value: u64) -> Result<()> {
  buf.write_u64::<LittleEndian>(value).map_err(Error::from)
}

#[inline]
fn read_u64<R: Read>(buf: &mut R) -> Result<u64> {
  buf.read_u64::<LittleEndian>().map_err(Error::from)
}

#[inline]
fn write_u128<W: Write>(buf: &mut W, value: u128) -> Result<()> {
  buf.write_u128::<LittleEndian>(value).map_err(Error::from)
}

#[inline]
fn read_u128<R: Read>(buf: &mut R) -> Result<u128> {
  buf.read_u128::<LittleEndian>().map_err(Error::from)
}

#[inline]
fn write_bin<W: Write>(buf: &mut W, value: &[u8]) -> Result<()> {
  write_u16(buf, value.len() as u16)?;
  buf.write_all(value).map_err(Error::from)
}

#[inline]
fn read_bin<R: Read>(buf: &mut R) -> Result<Vec<u8>> {
  let expected = read_u16(buf)? as usize;
  let mut buffer = Vec::<u8>::with_capacity(expected);
  unsafe {
    buffer.set_len(expected);
  }
  buf.read_exact(&mut buffer)?;
  Ok(buffer)
}
