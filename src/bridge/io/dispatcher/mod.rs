use std::collections::HashMap;
use std::future::Future;
use std::ops::{DerefMut};
use std::pin::Pin;
use std::sync::{Arc, Mutex, RwLock};
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Waker};
use std::thread::{JoinHandle, spawn};

use log;
use mio::{Events, Interest, Poll, Token};
use mio::event::Event;
use mio::net::{TcpListener, TcpStream};

use crate::error::Error;
use crate::Result;
use std::sync::mpsc::{Sender, channel, Receiver};

/// Poll に登録するためのソケットを格納する列挙型。
enum Socket {
  Waker,
  Stream(TcpStream),
  Listener(TcpListener),
}

/// オブジェクトに対する ID の割当と ID による参照操作を行うためのマップ。
/// Poll で通知されたトークンからソケットを特定するために使用します。
/// Note that this [IdMap] is not thread-safe.
struct SocketMap {
  next: usize,
  sockets: HashMap<usize, Arc<Mutex<Socket>>>,
}

impl SocketMap {
  /// 新規のマップを作成します。
  pub fn new() -> SocketMap {
    let sockets = HashMap::new();
    SocketMap { next: 0, sockets }
  }

  /// 指定された ID のオブジェクトを参照します。
  pub fn get(&self, id: usize) -> Option<Arc<Mutex<Socket>>> {
    if id == 0 {
      Some(Arc::new(Mutex::new(Socket::Waker)))
    } else {
      self.sockets.get(&id).map(|a| a.clone())
    }
  }

  /// 指定されたオブジェクトをこのマップに格納し、新しく発行された ID を返します。
  pub fn add(&mut self, socket: Socket) -> Result<usize> {
    // NOTE Token(0) は Waker 用、Token(usize::MAX) は Poll が内部的に使用している
    let max = std::usize::MAX - 2;
    if self.sockets.len() == max {
      return Err(Error::TooManySockets { maximum: std::usize::MAX });
    }
    for i in 0..=max {
      let id = (self.next as u64 + i as u64) as usize + 1;
      if self.sockets.get(&id).is_none() {
        self.sockets.insert(id, Arc::new(Mutex::new(socket)));
        self.next = if self.next + 1 == max { 0 } else { self.next + 1 };
        return Ok(id);
      }
    }
    unreachable!()
  }

  /// 指定された ID のオブジェクトをこのマップから削除して返します。
  pub fn remove(&mut self, id: usize) -> Option<Arc<Mutex<Socket>>> {
    self.sockets.remove(&id)
  }
}

pub struct Dispatcher {
  event_thread: JoinHandle<Result<()>>,
  sender: Sender<TASK>,
  state: Arc<DispatcherState>,
}

type TASK = Box<dyn FnMut(&mut Poll, &mut SocketMap) + Send>;

struct DispatcherState {
  is_closed: AtomicBool,
  receiver: Receiver<TASK>,
  waker: mio::Waker,
}

impl Dispatcher {
  pub fn new(event_buffer_size: usize) -> Result<Dispatcher> {
    let (sender, receiver) = channel()?;
    let poll = Mutex::new(Poll::new()?);
    let waker = mio::Waker::new(poll.lock()?.registry(), Token(0))?;
    let state = Arc::new(DispatcherState {
      is_closed: AtomicBool::new(false),
      receiver,
      waker,
    });

    let cloned_state = state.clone();
    let event_thread = spawn(move || {
      cloned_state.event_loop(event_buffer_size)
    });
    Ok(Dispatcher { event_thread, sender, state })
  }

  pub fn stop(&mut self) -> Result<()> {
    if self.state.is_closed.compare_and_swap(false, true, Ordering::SeqCst) {
      log::info!("stopping TCP bridge...");
      self.state.waker.wake()?;
      // self.event_thread.join()?
    }
    Ok(())
  }
}

impl DispatcherRegister<TcpListener> for Dispatcher {
  fn register(&mut self, mut listener: TcpListener) -> Pin<Box<dyn Future<Output=usize>>> {
    self.state.run_in_event_loop(|poll, sockets| {
      let id = sockets.add(Socket::Listener(listener))?;
      poll.registry().register(&mut listener, Token(id), Interest::READABLE).unwrap();
      id
    })
  }
}

// impl DispatcherRegister<TcpStream> for Dispatcher {
//   fn register(&mut self, mut stream: TcpStream) -> Pin<Box<dyn Future<Output=usize>>> {
//     self.state.run_in_event_loop(|poll, sockets| {
//       let id = sockets.add(socket)?;
//       poll.registry().register(&mut stream, Token(id), Interest::READABLE).unwrap();
//       id
//     })
//   }
// }

pub trait DispatcherRegister<T> {
  fn register(&mut self, stream: T) -> Pin<Box<dyn Future<Output=usize>>>;
}

impl DispatcherState {
  fn run_in_event_loop<R:'static>(&mut self, exec: fn(&mut Poll, &mut SocketMap) -> R) -> impl Future {
    let mut queue = self.queue.write().unwrap();
    let mut future = EventLoopResult::new();
    queue.push(Box::new(move |poll, sockets| {
      let result = exec(poll, sockets);
      future.complete(result);
    }));
    self.waker.wake();
    future
  }

  fn event_loop(&self, event_buffer_size: usize) -> Result<()> {
    let mut poll = Poll::new()?;
    let mut events = Events::with_capacity(event_buffer_size);
    let mut sockets = SocketMap::new();

    while self.is_closed.load(Ordering::SeqCst) {
      poll.poll(&mut events, None)?;

      // イベントの発生したソケットを取得
      let event_sockets = events.iter()
        .map(|e| sockets.get(e.token().0).map(|s| (e, s)))
        .flatten().collect::<Vec<(&Event, Arc<Mutex<Socket>>)>>();

      // イベントの発生したソケットの処理を実行
      for (event, socket) in event_sockets.iter() {
        match socket.lock()?.deref_mut() {
          Socket::Stream(stream) => {
            log::info!("CLIENT");
          }
          Socket::Listener(listener) => {
            log::info!("SERVER");
          }
          Socket::Waker => {
            log::info!("WAKER");
          }
        }
      }

      // キューに保存されている処理を実行
      let mut queue = self.queue.write()?;
      while let Some(exec) = queue.pop() {
        exec(&mut poll, &mut sockets);
      }
    }
    Ok(())
  }

  /// キューに保存されている登録待ちのソケットを Poll に登録します。
  fn register_all(&self, poll: &mut Poll) -> Result<()> {
    let mut queue = self.queue.write()?;
    let sockets = self.sockets.read()?;
    while let Some((token, socket)) = queue.pop() {
      match socket.lock()?.deref_mut() {
        Socket::Stream(mut stream) => {
          poll.registry().register(&mut stream, token, Interest::READABLE | Interest::WRITABLE)?;
        }
        Socket::Listener(mut listener) => {
          poll.registry().register(&mut listener, token, Interest::READABLE)?;
        }
        Socket::Waker => assert!(false, "Waker was inserted in queue")
      }
    }
    Ok(())
  }

  /// 登録されているすべてのソケットを解除します。
  fn deregister_all(&self) -> Result<()> {
    // self.poll.registry().deregister()
    Ok(())
  }
}

struct EventLoopResult<R> {
  result: RwLock<Vec<R>>,
  waker: Option<Waker>,
}

impl<R> EventLoopResult<R> {
  fn new() -> EventLoopResult<R> {
    EventLoopResult {
      result: RwLock::new(Vec::with_capacity(1)),
      waker: None,
    }
  }
  fn complete(&mut self, result: R) {
    let mut buffer = self.result.write().unwrap();
    if buffer.is_empty() {
      buffer.push(result);
    }
  }
}

impl<R> Future for EventLoopResult<R> {
  type Output = R;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> std::task::Poll<R> {
    let buffer = self.result.read().unwrap();
    if buffer.is_empty() {
      if self.waker.is_none() {
        self.waker = Some(cx.waker().clone());
      }
      std::task::Poll::Pending
    } else {
      std::task::Poll::Ready(buffer.get(0).unwrap())
    }
  }
}
