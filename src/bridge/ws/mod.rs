use std::future::Future;
use std::net::TcpListener;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::thread::{JoinHandle, spawn};

use log;
use tungstenite::server::accept;
use url::Url;

use crate::error::Error;
use crate::Result;

pub struct WsServer {
  name: String,
  url: Url,
  address: String,
  server: TcpListener,
  handle: Arc<JoinHandle<()>>,
}

impl WsServer {
  pub fn listen(&mut self) -> WsFuture {
    unimplemented!()
  }
}

struct WsFuture {
  server: Arc<WsServer>,
}

impl Future for WsFuture {
  type Output = Result<()>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {

    // バインドアドレスを取得
    let host = self.server.url.host_str();
    let port = self.server.url.port();
    let address = if let (Some(host), Some(port)) = (host, port) {
      format!("{0}:{1}", host, port)
    } else {
      return Poll::Ready(Err(Error::HostNotSpecifiedInUrl { url: self.server.url.to_string() }));
    };

    // バインドの実行
    let name = &self.server.name;
    log::debug!("{} is trying to start a WebSocket service at address: {}", name, address);
    let server = match TcpListener::bind(&address) {
      Ok(server) => server,
      Err(err) => return Poll::Ready(Err(From::from(err)))
    };
    log::info!("{} has started a WebSocket service at address: {}", name,
               server.local_addr().map(|addr| format!("{}:{}", addr.ip().to_string(), addr.port()).to_string()).unwrap_or(address));

    // TODO It should be possible to specify threads or thread pools externally.
    self.server.handle = Arc::new(spawn(move || loop {
      match server.accept() {
        Ok((stream, addr)) => {}
        Err(err) => {
          break;
        }
      }
    }));
    Poll::Ready(Ok(()))
  }
}
