pub mod msgpack;

/**
シリアライズした 1 メッセージの最大バイナリ長です。IPv4 のデータ部最大長である 65,507 を表します。
*/
pub const MAX_MESSAGE_SIZE: usize = 65507;

/*
メッセージのシリアライズとデシリアライズを行うためのクラスです。
アプリケーションインターフェースで使用されているパラメータのシリアライズ/デシリアライズは Marshal と Unmarshal
によってサブクラスの実装に委譲されています。
*/
// pub trait Codec {
//   fn encode(msg:&dyn Message) -> &[u8];
//   fn decode(buffer:&[u8]) -> Result<&dyn Message>;
// }
