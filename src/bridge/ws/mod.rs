use std::future::Future;
use std::net::TcpListener;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
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
  closed: AtomicBool,
}

impl WsServer {
  pub fn listen<F: Future<Output=Result<WsServer>>>(name: &str, url: Url) -> F {

    // バインドアドレスを構築
    let host = url.host_str();
    let port = url.port();
    let address = if let (Some(host), Some(port)) = (host, port) {
      format!("{0}:{1}", host, port)
    } else {
      return Poll::Ready(Err(Error::HostNotSpecifiedInUrl { url: url.to_string() }));
    };

    // 非同期で bind して WebSocket サーバとして返す
    async {
      log::debug!("{} is trying to start a WebSocket service at address: {}", name, address);
      let server = match TcpListener::bind(&address) {
        Ok(server) => server,
        Err(err) => {
          log::error!("{} was failed to start a WebSocket service at address: {}", name, address);
          return Err(From::from(err));
        }
      };
      let address = server.local_addr()
        .map(|addr| format!("{}:{}", addr.ip().to_string(), addr.port()).to_string())
        .unwrap_or(address);
      log::info!("{} has started a WebSocket service at address: {}", name, address);
      Ok(WsServer { name: name.to_string(), url, address, server, closed: AtomicBool::new(false) })
    }
  }

  pub fn close(&mut self) -> () {
    if self.closed.compare_and_swap(false, true) {
      self.server
    }
  }

  pub fn accept(&mut self) -> () {
    // TODO It should be possible to specify threads or thread pools externally.
    Arc::new(spawn(move || loop {
      match self.server.accept() {
        Ok((stream, addr)) => {}
        Err(err) => {
          break;
        }
      }
    }));
  }
}
