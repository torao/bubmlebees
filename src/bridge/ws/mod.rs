use std::future::Future;
use std::net::TcpListener;
use std::thread::spawn;

use tungstenite::server::accept;
use url::Url;

use crate::error::Error;
use crate::Result;

pub struct WsServer {
  address: String,
  server: TcpListener,
}

pub async fn listen(url: &Url) -> Result<()> {
  let address = if let (Some(host), Some(port)) = (url.host_str(), url.port()) {
    format!("{0}:{1}", host, port)
  } else {
    return Err(Error::HostNotSpecifiedInUrl { url: url.to_string() });
  };
  let server = TcpListener::bind(address)?;
  loop {
    match server.accept() {
      Ok((stream, addr)) => {}
      Err(err) => {
        break;
      }
    }
  }
  Ok(())
}
