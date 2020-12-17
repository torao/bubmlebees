use std::net::{Shutdown, SocketAddr};

use async_trait::async_trait;
use log;
use mio::net::{TcpListener, TcpStream};
use url::Url;

use crate::bridge::{Bridge, Server, Wire};
use crate::bridge::io::dispatcher::{Dispatcher, DispatcherRegister};
use crate::Result;

#[cfg(test)]
mod test;

pub struct TcpBridge {
  dispatcher: Dispatcher,
}

impl TcpBridge {
  pub fn new(event_buffer_size: usize) -> Result<TcpBridge> {
    Ok(TcpBridge {
      dispatcher: Dispatcher::new(event_buffer_size)?
    })
  }

  pub fn stop(&mut self) -> Result<()> {
    self.dispatcher.stop()
  }
}

#[async_trait]
impl Bridge<TcpServer> for TcpBridge {
  fn name(&self) -> &'static str {
    "tcp"
  }

  ///  指定されたリモートノードに対して非同期接続を行い `Wire` の Future を返します。
  fn new_wire<W: Wire>(&mut self) -> Result<W> {
    unimplemented!()
  }

  /// 指定されたネットワークからの接続を非同期で受け付ける `Server` の Future を返します。
  async fn start_server(&mut self, url: &Url) -> Result<TcpServer> {
    assert_eq!(url.scheme(), self.name());
    let bind_address = if let (Some(host), Some(port)) = (url.host_str(), url.port()) {
      format!("{}:{}", host, port)
    } else {
      url.host_str().unwrap_or("localhost").to_string()
    };
    let bind_address = bind_address.parse()?;

    // 新しい TcpListener の登録
    let listener = TcpListener::bind(bind_address)?;
    let url = listener.local_addr()
      .map(|addr| format!("{}://{}", self.name(), addr.to_string()))
      .unwrap_or("<unknown>".to_string());
    let id = self.dispatcher.register(listener)?;

    Ok(TcpServer { id, url })
  }
}

struct TcpWire {
  is_server: bool,
  client: TcpStream,
}

impl Wire for TcpWire {
  fn local_address(&self) -> Result<SocketAddr> {
    self.client.local_addr().map_err(From::from)
  }

  fn remote_address(&self) -> Result<SocketAddr> {
    self.client.peer_addr().map_err(From::from)
  }

  fn is_server(&self) -> bool {
    self.is_server
  }

  fn close(&mut self) -> Result<()> {
    self.client.shutdown(Shutdown::Both).map_err(From::from)
  }
}

struct TcpServer {
  id: usize,
  url: String,
}

impl Server for TcpServer {
  fn url(&self) -> &str {
    &self.url
  }
  fn close(&mut self) -> Result<()> {
    unimplemented!()
  }
}


/*
pub struct Server {
  name: String,
  url: Url,
  address: String,
  server: Option<TcpListener>,
  closed: AtomicBool,
}

impl Server {
  pub async fn listen(name: &str, url: Url) -> Result<Server> {
    // バインドアドレスを構築
    let host = url.host_str();
    let port = url.port();
    let address = if let (Some(host), Some(port)) = (host, port) {
      format!("{0}:{1}", host, port)
    } else {
      return Err(Error::HostNotSpecifiedInUrl { url: url.to_string() });
    };

    // 非同期で bind して WebSocket サーバとして返す
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
    let server = Some(server);
    Ok(Server { name: name.to_string(), url, address, server, closed: AtomicBool::new(false) })
  }

  pub fn close(&mut self) -> () {
    if self.closed.compare_and_swap(false, true, Ordering::Relaxed) {
      self.server = None;
    }
  }

  pub fn accept(&mut self) -> () {
    // TODO It should be possible to specify threads or thread pools externally.
    Arc::new(spawn(move || loop {
      if let Some(server) = self.server {
        match self.server.accept() {
          Ok((stream, addr)) => {}
          Err(err) => {
            break;
          }
        }
      } else {
        break;
      }
    }));
  }
}
*/