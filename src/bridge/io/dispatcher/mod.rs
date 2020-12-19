use std::collections::HashMap;
use std::future::Future;
use std::ops::DerefMut;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::task::{Context, Waker};
use std::thread::{JoinHandle, spawn};

use log;
use mio::{Events, Interest, Poll, Token};
use mio::event::Event;
use mio::net::{TcpListener, TcpStream};

use crate::error::Error;
use crate::Result;

type Executable<R> = dyn (FnMut(&mut Poll, &mut SocketMap) -> R) + Send + 'static;

struct TaskState<R> {
  result: Option<R>,
  waker: Option<Waker>,
}

struct Task<R> {
  executable: Box<Executable<R>>,
  state: Arc<Mutex<TaskState<R>>>,
}

impl<R> Task<R> {
  fn new<E>(executable: E) -> Self where E: (FnMut(&mut Poll, &mut SocketMap) -> R) + Send + 'static {
    Self {
      executable: Box::new(executable),
      state: Arc::new(Mutex::new(TaskState {
        result: None,
        waker: None,
      })),
    }
  }
  fn execute(&mut self, poll: &mut Poll, sockets: &mut SocketMap) {
    let result = (self.executable)(poll, sockets);
    let mut state = self.state.lock().unwrap();
    state.result = Some(result);
    if let Some(waker) = state.waker.take() {
      waker.wake();
    }
  }
}

struct TaskFuture<R> {
  state: Arc<Mutex<TaskState<R>>>
}

impl<R> Future for TaskFuture<R> {
  type Output = R;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> std::task::Poll<Self::Output> {
    use std::task::Poll;
    let mut state = self.state.lock().unwrap();
    if let Some(result) = state.result.take() {
      Poll::Ready(result)
    } else {
      state.waker = Some(cx.waker().clone());
      Poll::Pending
    }
  }
}

pub type SocketId = usize;

pub struct Dispatcher {
  event_thread: JoinHandle<Result<()>>,
  sender: Sender<Task<Result<SocketId>>>,
  waker: mio::Waker,
  is_closed: Arc<AtomicBool>,
}

impl Drop for Dispatcher {
  fn drop(&mut self) {
    self.stop().unwrap();
  }
}

impl Dispatcher {
  pub fn new(event_buffer_size: usize) -> Result<Dispatcher> {
    let (sender, receiver) = channel();
    let mut poll = Poll::new()?;
    let waker = mio::Waker::new(poll.registry(), Token(0))?;

    let is_closed = Arc::new(AtomicBool::new(false));
    let is_closed_cloned = is_closed.clone();
    let event_thread = spawn(move || {
      Dispatcher::event_loop(receiver, poll, is_closed_cloned, event_buffer_size)
    });
    Ok(Dispatcher { event_thread, sender, waker, is_closed })
  }

  pub fn stop(&mut self) -> Result<()> {
    if self.is_closed.compare_and_swap(false, true, Ordering::SeqCst) {
      log::info!("stopping TCP bridge...");
      self.waker.wake()?;
    }
    Ok(())
  }

  fn run_in_event_loop(&mut self, exec: fn(&mut Poll, &mut SocketMap) -> Result<SocketId>) -> TaskFuture<Result<SocketId>> {
    let task = Task::new(exec);
    let future = TaskFuture { state: task.state.clone() };
    self.sender.send(task).unwrap();
    self.waker.wake().unwrap();
    future
  }

  fn event_loop<R>(receiver: Receiver<Task<R>>, mut poll: Poll, is_closed: Arc<AtomicBool>, event_buffer_size: usize) -> Result<()> {
    let mut events = Events::with_capacity(event_buffer_size);
    let mut sockets = SocketMap::new();

    while is_closed.load(Ordering::SeqCst) {
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

      // タスクキューに入っている処理をすべて実行
      for mut task in receiver.iter() {
        task.execute(&mut poll, &mut sockets);
      }
    }
    Ok(())
  }
}

pub trait DispatcherRegister<T> {
  fn register<F>(&mut self, stream: T) -> F where F: Future<Output=Result<SocketId>>;
}

impl DispatcherRegister<TcpListener> for Dispatcher {
  fn register<F>(&mut self, mut listener: TcpListener) -> F where F: Future<Output=Result<SocketId>> {
    self.run_in_event_loop(move |poll, sockets| {
      let id = sockets.add(Socket::Listener(listener))?;
      poll.registry()
        .register(&mut listener, Token(id), Interest::READABLE)?;
      Ok(id)
    })
  }
}

impl DispatcherRegister<TcpStream> for Dispatcher {
  fn register<F>(&mut self, mut stream: TcpStream) -> F where F: Future<Output=Result<SocketId>> {
    self.run_in_event_loop(move |poll, sockets| {
      let id = sockets.add(Socket::Stream(stream))?;
      poll.registry()
        .register(&mut stream, Token(id), Interest::READABLE | Interest::WRITABLE)?;
      Ok(id)
    })
  }
}

/// Poll に登録するソケットを格納する列挙型。
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
  pub fn add(&mut self, socket: Socket) -> Result<SocketId> {
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
