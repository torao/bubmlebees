use rand;
use rand::prelude::StdRng;
use rand::{RngCore, SeedableRng};

/// 一様にランダムなテスト用の値を採集するための構造体。シードを指定することでランダムだが決定論的な値を生成する。
pub struct SampleValues {
  rng: Box<StdRng>,
}

impl SampleValues {
  /// シードを指定してサンプル値ジェネレータを初期化します。
  pub fn new(seed: u64) -> SampleValues {
    let mut s = [0u8; 32];
    for i in 0..8 {
      s[i] = ((seed >> (i * 8)) & 0xFF) as u8
    }
    SampleValues { rng: Box::new(rand::rngs::StdRng::from_seed(s)) }
  }

  pub fn next_bool(&mut self) -> bool {
    (self.rng.next_u32() & 0x01) != 0
  }

  pub fn next_u8(&mut self) -> u8 {
    (self.rng.next_u32() & 0xFF) as u8
  }

  pub fn next_u16(&mut self) -> u16 {
    (self.rng.next_u32() & 0xFFFF) as u16
  }

  pub fn next_bytes(&mut self, length: usize) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(length);
    for i in 0..length {
      bytes.push(0u8)
    }
    self.rng.fill_bytes(&mut bytes);
    bytes
  }
}

#[test]
fn test_sample_values() {
  let mut sample = SampleValues::new(783629830u64);
  assert_eq!(sample.next_bytes(1024).len(), 1024);
}
