use std::net::{Shutdown, SocketAddr};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{JoinHandle, spawn};
use std::time::Duration;

use log;
use mio::{Events, Interest, Poll, Token};
use mio::net::{TcpListener, TcpStream};

use crate::bridge::Bridge;
use crate::Result;

struct TcpBridge {
  poll: Arc<Mutex<Poll>>,
  event_dispatcher: JoinHandle<Result<()>>,
  is_closed: Arc<AtomicBool>,
}

impl TcpBridge {
  pub fn new(event_buffer_size: usize) -> Result<TcpBridge> {
    let poll = Arc::new(Mutex::new(Poll::new()?));
    let cloned_poll = poll.clone();
    let is_closed = Arc::new(AtomicBool::new(false));
    let cloned_is_closed = is_closed.clone();
    let event_dispatcher = spawn(move || {
      TcpBridge::start_event_loop(cloned_is_closed, cloned_poll, event_buffer_size)
    });
    Ok(TcpBridge { poll, event_dispatcher, is_closed })
  }

  pub fn stop(&mut self) -> () {
    if self.is_closed.compare_and_swap(false, true, Ordering::Relaxed) {
      log::info!("stopping TCP bridge...");
    }
  }

  fn start_event_loop(is_closed: Arc<AtomicBool>, poll: Arc<Mutex<Poll>>, event_buffer_size: usize) -> Result<()> {
    let mut events = Events::with_capacity(event_buffer_size);
    while is_closed.load(Ordering::Relaxed) {
      poll.lock()?.poll(&mut events, Some(Duration::from_secs(1)))?;
      for event in events.iter() {
        match event.token() {
          SERVER => {
            log::info!("SERVER");
          }
          CLIENT => {
            log::info!("CLIENT");
          }
          _ => unreachable!()
        }
      }
    }
    Ok(())
  }
}

const SERVER: Token = Token(0);
const CLIENT: Token = Token(1);

impl Bridge for TcpBridge {
  ///  指定されたリモートノードに対して非同期接続を行い `Wire` の Future を返します。
  fn new_wire<W: crate::bridge::Wire>(&mut self) -> Result<W> {
    unimplemented!()
  }

  /// 指定されたネットワークからの接続を非同期で受け付ける `Server` の Future を返します。
  fn new_server<S: crate::bridge::Server>(&mut self, bind_address: SocketAddr) -> Result<S> {
    let mut server = TcpListener::bind(bind_address)?;
    self.poll.lock()?.registry().register(&mut server, SERVER, Interest::READABLE)?;
    unimplemented!()
  }
}

struct Wire {
  is_server: bool,
  client: TcpStream,
}

impl crate::bridge::Wire for Wire {
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

pub struct Server {
  server: TcpListener
}

impl crate::bridge::Server for Server {
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