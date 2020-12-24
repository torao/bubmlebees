use std::net::SocketAddr;
use std::thread::spawn;

use byteorder::{ReadBytesExt, WriteBytesExt};
use mio::net::TcpListener;

use crate::bridge::io::dispatcher::Dispatcher;

#[test]
fn test_dispatcher() {
  let port = echo_server("hello, world", 1);
  println!("port: {}", port);
//  let dispatcher = Dispatcher::new(1024);
}


fn echo_server(expected: &'static str, clients:usize) -> u16 {
  let address = "localhost:0".parse::<SocketAddr>().unwrap();
  let listener = TcpListener::bind(address).unwrap();
  let port = listener.local_addr().unwrap().port();
  spawn(move || {
    for _ in 0..clients {
      let (mut stream, address) = listener.accept().unwrap();
      for expected in expected.chars().map(|c| c as u8) {
        let actual = stream.read_u8().unwrap();
        assert_eq!(expected, actual);
        stream.write_u8(actual).unwrap();
      }
      stream.read_u8().unwrap();
    }
  });
  port
}