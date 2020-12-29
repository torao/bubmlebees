use std::net::{SocketAddr, IpAddr, Ipv4Addr};
use std::thread::spawn;

use byteorder::{ReadBytesExt, WriteBytesExt};
use mio::net::{TcpListener, TcpStream};

use crate::bridge::io::dispatcher::{Dispatcher, TcpStreamListener, DispatcherAction};
use std::io::{Read, Write};
use mio::Interest;

#[test]
fn test_dispatcher() {
  let dispatcher = Dispatcher::new(1024).unwrap();

  let address = echo_server("hello, world", 1);
  println!("address: {}", address);

  let stream = TcpStream::connect(accress).unwrap();
  dispatcher.register(stream, Box::new()
}

struct EchoClient {
  buffer: &'static str,
  position: usize,
  echo_back: Box<[u8]>,
}

impl EchoClient {
  fn new(message: &'static str) -> EchoClient {
    EchoClient { buffer: message, position: 0, echo_back: Box::new()}
  }
}

impl TcpStreamListener for EchoClient {
  fn on_ready_to_read(&mut self, r: &mut dyn Read) -> DispatcherAction {
    println!("EchoClient::on_ready_to_read()");
  }
  fn on_ready_to_write(&mut self, w: &mut dyn Write) -> DispatcherAction {
    println!("EchoClient::on_ready_to_write()");
    let len = w.write(buffer[position..]).unwrap();
    self.position += len;
    if self.position == self.buffer.len() {
      DispatcherAction::ChangeFlag(Interest::READABLE)
    } else {
      DispatcherAction::Continue
    }
  }
  fn on_error(&mut self, error: std::io::Error) -> DispatcherAction {
    println!("EchoClient::on_error({})", error);
    DispatcherAction::Dispose
  }
}

fn echo_server(expected: &'static str, clients: usize) -> SocketAddr {
  let ip_address = IpAddr::from(Ipv4Addr::new(127, 0, 0, 1));
  let address = SocketAddr::new(ip_address, 0);
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
  SocketAddr::new(ip_address, port)
}