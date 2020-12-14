use std::collections::HashMap;
use std::net::{Shutdown, SocketAddr};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{JoinHandle, spawn};
use std::time::Duration;

use async_trait::async_trait;
use log;
use mio::{Events, Interest, Poll, Token};
use mio::net::{TcpListener, TcpStream};
use url::Url;

use crate::bridge::{Bridge, Server, Wire};
use crate::error::Error;
use crate::Result;

#[cfg(test)]
mod test;

struct TokenMap<T> {
  next: usize,
  values: HashMap<usize, T>,
}

impl<T> TokenMap<T> {
  pub fn new() -> TokenMap<T> {
    TokenMap { next: 0, values: HashMap::new() }
  }
  pub fn get(&self, index: usize) -> Option<&T> {
    self.values.get(&index)
  }
  pub fn add(&mut self, value: T) -> Result<usize> {
    if self.values.len() == std::usize::MAX {
      return Err(Error::TooManySockets { maximum: std::usize::MAX });
    }
    // NOTE token cannot be Token(usize::MAX) as it is reserved for internal usage.
    for i in 0..std::usize::MAX {
      let index = (self.next as u64 + i as u64) as usize;
      if self.values.get(&index).is_none() {
        self.values.insert(index, value);
        return Ok(index);
      }
    }
    unreachable!()
  }
  pub fn remove(&mut self, index: usize) -> Option<T> {
    self.values.remove(&index)
  }
}

enum TcpSocket {
  Stream(TcpStream),
  Listener(TcpListener),
}

pub struct TcpBridge {
  poll: Arc<Mutex<Poll>>,
  event_dispatcher: JoinHandle<Result<()>>,
  is_closed: Arc<AtomicBool>,
  sockets: Arc<TokenMap<TcpSocket>>,
}

impl TcpBridge {
  pub fn new(event_buffer_size: usize) -> Result<TcpBridge> {
    let poll = Arc::new(Mutex::new(Poll::new()?));
    let cloned_poll = poll.clone();
    let is_closed = Arc::new(AtomicBool::new(false));
    let cloned_is_closed = is_closed.clone();
    let sockets = Arc::new(TokenMap::new());
    let cloned_sockets = sockets.clone();
    let event_dispatcher = spawn(move || {
      TcpBridge::start_event_loop(cloned_is_closed, cloned_poll, cloned_sockets, event_buffer_size)
    });
    Ok(TcpBridge { poll, event_dispatcher, is_closed, sockets })
  }

  pub fn stop(&mut self) -> () {
    if self.is_closed.compare_and_swap(false, true, Ordering::Relaxed) {
      log::info!("stopping TCP bridge...");
      // TODO 残っているすべてのソケットをクローズ
    }
    self.event_dispatcher.interr
  }

  fn start_event_loop(is_closed: Arc<AtomicBool>, poll: Arc<Mutex<Poll>>, tokens: Arc<TokenMap<TcpSocket>>, event_buffer_size: usize) -> Result<()> {
    let mut events = Events::with_capacity(event_buffer_size);
    while is_closed.load(Ordering::Relaxed) {
      let sockets = {
        let mut poll = poll.lock()?;
        poll.poll(&mut events, Some(Duration::from_secs(1)))?;
        let tokens = tokens.clone();
        events.iter().map(|e| tokens.get(e.token().0)).collect::<Vec<TcpSocket>>()
      };
      for socket in sockets.iter() {
        match socket {
          TcpSocket::Stream(stream) => {
            log::info!("CLIENT");
          }
          TcpSocket::Listener(listener) => {
            log::info!("SERVER");
          }
          _ => unreachable!()
        }
      }
    }
    Ok(())
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
    let id = {
      let mut listener = TcpListener::bind(bind_address)?;
      let poll = self.poll.lock()?;
      let id = self.sockets.add(TcpSocket::Listener(listener))?;
      poll.registry()
        .register(&mut listener, Token(id), Interest::READABLE)
        .map_err(From::from);
      id
    };

    Ok(TcpServer { id, local_address: lis })
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
  local_address: Result<String>,
}

impl Server for TcpServer {
  fn local_address(&self) -> Result<String> {
    self.server.local_addr().map(|addr| format!("tcp://{}", addr.to_string())).map_err(From::from)
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