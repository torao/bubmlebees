use crate::bridge::tcp::TcpBridge;
use crate::bridge::Bridge;

#[test]
fn test_tcp_bridge() {
  let mut bridge = TcpBridge::new(1024).unwrap();
  let mut server = bridge.start_server()?;
}