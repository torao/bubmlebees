use std::collections::HashMap;
use std::future::Future;
use std::io::{Read, Write};
use std::net::SocketAddr;
use std::ops::DerefMut;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::task::{Context, Waker};
use std::thread::spawn;

use log;
use mio::{Events, Interest, Poll, Token};
use mio::event::{Event, Source};
use mio::net::{TcpListener, TcpStream};

use crate::error::Error;
use crate::Result;

/// TcpStream にイベントが発生したときに呼び出されるコールバック用のトレイトです。
/// 返値を使用してその後のアクションを指定することができます。
pub trait TcpStreamListener: Send {
  fn on_ready_to_read(&mut self, r: &mut dyn Read) -> DispatcherAction;
  fn on_ready_to_write(&mut self, w: &mut dyn Write) -> DispatcherAction;
  fn on_error(&mut self, error: std::io::Error) -> DispatcherAction;
}

/// TcpListener にイベントが発生したときに呼び出されるコールバック用のトレイトです。
/// 返値を使用してその後のアクションを指定することができます。
pub trait TcpListenerListener: Send {
  fn on_accept(&mut self, stream: TcpStream, address: SocketAddr) -> DispatcherAction;
  fn on_error(&mut self, error: std::io::Error) -> DispatcherAction;
}

/// Listener へのコールバック終了後に Listener が Dispatcher に指示する動作を表す列挙型です。
pub enum DispatcherAction {
  /// 特に何も行わないで処理を続行することを示します。
  Continue,
  /// 指定された Interest フラグに変更することを指定します。
  ChangeFlag(Interest),
  /// イベントの発生元となるソケットなどの Source の破棄を指定します。
  Dispose,
}

// ##############################################################################################
// イベントループスレッド内で外部の指定した処理を行うために channel 経由で送受信されるタスクとその結果を返す Future
// の定義。

type Executable<R> = dyn (FnOnce(&mut PollingLoop) -> R) + Send + 'static;

struct TaskState<R> {
  result: Option<R>,
  waker: Option<Waker>,
}

struct Task<R> {
  executable: Box<Executable<R>>,
  state: Arc<Mutex<TaskState<R>>>,
}

impl<R> Task<R> {
  fn new<E>(executable: Box<E>) -> Self
    where
      E: (FnOnce(&mut PollingLoop) -> R) + Send + 'static,
  {
    Self { executable, state: Arc::new(Mutex::new(TaskState { result: None, waker: None })) }
  }
}

pub struct TaskFuture<R> {
  state: Arc<Mutex<TaskState<R>>>,
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

// ##############################################################################################

pub type SocketId = usize;

pub struct Dispatcher {
  sender: Sender<Task<Result<SocketId>>>,
  closed: AtomicBool,
  waker: mio::Waker,
}

impl Drop for Dispatcher {
  #[warn(unused_must_use)]
  fn drop(&mut self) {
    self.stop();
  }
}

impl Dispatcher {
  pub fn new(event_buffer_size: usize) -> Result<Dispatcher> {
    let (sender, receiver) = channel();
    let poll = Poll::new()?;
    let waker = mio::Waker::new(poll.registry(), Token(0))?;
    let mut polling_loop = PollingLoop::new(poll, event_buffer_size);
    spawn(move || polling_loop.start(receiver));
    let closed = AtomicBool::new(false);
    Ok(Dispatcher { sender, closed, waker })
  }

  pub fn stop(&mut self) -> Box<dyn Future<Output=Result<SocketId>>> {
    if self.closed.compare_and_swap(false, true, Ordering::SeqCst) {
      log::debug!("stopping dispatcher...");
      self.run_in_event_loop(Box::new(move |polling: &mut PollingLoop| {
        polling.closed = true;
        Ok(0usize)
      }))
    } else {
      Box::new(std::future::ready(Ok(0usize)))
    }
  }

  fn run_in_event_loop<E>(&mut self, exec: Box<E>) -> Box<dyn Future<Output=Result<SocketId>>>
    where
      E: (FnOnce(&mut PollingLoop) -> Result<SocketId>) + Send + 'static,
  {
    let task = Task::new(exec);
    let future = TaskFuture { state: task.state.clone() };
    self.sender.send(task).unwrap();
    self.waker.wake().unwrap();
    Box::new(future)
  }
}

struct PollingLoop {
  poll: Poll,
  event_buffer_size: usize,
  sockets: SocketMap,
  closed: bool,
}

impl PollingLoop {
  fn new(poll: Poll, event_buffer_size: usize) -> PollingLoop {
    let sockets = SocketMap::new();
    PollingLoop { poll, event_buffer_size, sockets, closed: false }
  }

  /// poll() のためのイベントループを開始します。イベントループスレッドの中で任意の処理を行う場合は receiver に対応
  /// する sender に実行するタスクを投入し、self.poll に登録済みの Waker.wake() でブロッキングを抜けます。
  fn start<R>(&mut self, receiver: Receiver<Task<Result<R>>>) -> Result<()> {
    let mut events = Events::with_capacity(self.event_buffer_size);
    while !self.closed {
      self.poll.poll(&mut events, None)?;

      // イベントの発生したソケットを取得
      let event_sockets = events
        .iter()
        .map(|e| self.sockets.get(e.token().0).map(|s| (e, s)))
        .flatten()
        .collect::<Vec<(&Event, Arc<Mutex<Socket>>)>>();

      // イベントの発生したソケットの処理を実行
      for (event, socket) in event_sockets.iter() {
        match socket.lock()?.deref_mut() {
          Socket::Stream(stream, listener) => {
            log::info!("CLIENT[{}]", event.token().0);
            self.on_tcp_stream(event, stream, listener);
          }
          Socket::Listener(listener, event_listener) => {
            log::info!("SERVER[{}]", event.token().0);
            self.on_tcp_listener(event, listener, event_listener);
          }
          Socket::Waker => {
            log::info!("WAKER");
          }
        }
      }

      self.run_all_tasks(&receiver);
    }

    self.cleanup();
    log::info!("dispatcher stopped");
    Ok(())
  }

  /// 指定された receiver に存在するすべてのタスクを実行します。
  fn run_all_tasks<R>(&mut self, receiver: &Receiver<Task<Result<R>>>) {
    for Task { executable, state } in receiver.iter() {
      let result = executable(self);
      let mut state = state.lock().unwrap();
      state.result = Some(result);
      if let Some(waker) = state.waker.take() {
        waker.wake();
      }
    }
  }

  /// 指定された ID のソケットを廃棄します。この操作により対応するソケットはクローズします。
  fn close(&mut self, id: SocketId) {
    if let Some(socket) = self.sockets.sockets.remove(&id) {
      log::debug!("closing socket: {}", id);
      match socket.lock().unwrap().deref_mut() {
        Socket::Waker => (),
        Socket::Stream(stream, _) => self.poll.registry().deregister(stream).unwrap(),
        Socket::Listener(listener, _) => self.poll.registry().deregister(listener).unwrap(),
      };
      log::debug!("socket closed: {}", id);
    }
  }

  /// 登録されているすべてのソケットを廃棄します。この操作によりソケットはクローズされます。
  fn cleanup(&mut self) {
    for id in self.sockets.ids() {
      self.close(id);
    }
  }

  fn action<S: Source>(&mut self, id: SocketId, source: &mut S, action: DispatcherAction) {
    match action {
      DispatcherAction::Continue => (),
      DispatcherAction::ChangeFlag(interest) => {
        self.poll.registry().reregister(source, Token(id), interest).unwrap();
      }
      DispatcherAction::Dispose => self.close(id),
    }
  }

  fn on_tcp_stream(
    &mut self,
    event: &Event,
    stream: &mut TcpStream,
    listener: &mut Box<dyn TcpStreamListener>,
  ) {
    // 読み込み可能イベント
    if event.is_readable() {
      let behaviour = listener.on_ready_to_read(stream);
      self.action(event.token().0, stream, behaviour);
    }

    // 書き込み可能イベント
    if event.is_writable() {
      let behaviour = listener.on_ready_to_write(stream);
      self.action(event.token().0, stream, behaviour);
    }

    if event.is_error() {
      let behaviour = match stream.take_error() {
        Ok(Some(err)) => listener.on_error(err),
        Ok(None) => DispatcherAction::Continue,
        Err(err) => listener.on_error(err),
      };
      self.action(event.token().0, stream, behaviour);
    }
  }

  fn on_tcp_listener(
    &mut self,
    event: &Event,
    listener: &mut TcpListener,
    event_listener: &mut Box<dyn TcpListenerListener>,
  ) {
    // ソケット接続イベント
    if event.is_readable() {
      let (stream, address) = listener.accept().unwrap();
      let behaviour = event_listener.on_accept(stream, address);
      self.action(event.token().0, listener, behaviour);
    }
  }
}

trait DispatcherRegister<S, L> {
  fn register(&mut self, source: S, listener: L) -> Box<dyn Future<Output=Result<SocketId>>>;
}

impl DispatcherRegister<TcpListener, Box<dyn TcpListenerListener>> for Dispatcher {
  fn register(
    &mut self,
    mut listener: TcpListener,
    event_listener: Box<dyn TcpListenerListener>,
  ) -> Box<dyn Future<Output=Result<SocketId>>> {
    self.run_in_event_loop(Box::new(move |polling: &mut PollingLoop| {
      let id = polling.sockets.available_id()?;
      polling.poll.registry().register(&mut listener, Token(id), Interest::READABLE)?;
      polling.sockets.set(id, Socket::Listener(listener, event_listener));
      Ok(id)
    }))
  }
}

impl DispatcherRegister<TcpStream, Box<dyn TcpStreamListener>> for Dispatcher {
  fn register(
    &mut self,
    mut stream: TcpStream,
    listener: Box<dyn TcpStreamListener>,
  ) -> Box<dyn Future<Output=Result<SocketId>>> {
    self.run_in_event_loop(Box::new(move |polling: &mut PollingLoop| {
      let id = polling.sockets.available_id()?;
      polling.poll.registry().register(
        &mut stream,
        Token(id),
        Interest::READABLE | Interest::WRITABLE,
      )?;
      polling.sockets.set(id, Socket::Stream(stream, listener));
      Ok(id)
    }))
  }
}

/// Poll に登録するソケットを格納する列挙型。
enum Socket {
  Waker,
  Stream(TcpStream, Box<dyn TcpStreamListener>),
  Listener(TcpListener, Box<dyn TcpListenerListener>),
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

  /// 管理されているすべての ID を参照します。
  pub fn ids(&self) -> Vec<SocketId> {
    self.sockets.keys().map(|id| *id).collect::<Vec<usize>>()
  }

  /// 使用可能な ID を検索します。
  pub fn available_id(&mut self) -> Result<SocketId> {
    // NOTE: Token(0) は Waker 用、Token(usize::MAX) は Poll が内部的に使用しているためそれぞれ予約されている
    let max = std::usize::MAX - 2;
    if self.sockets.len() == max {
      return Err(Error::TooManySockets { maximum: std::usize::MAX });
    }
    for i in 0..=max {
      let id = (self.next as u64 + i as u64) as usize + 1;
      if self.sockets.get(&id).is_none() {
        self.next = if self.next + 1 == max { 0 } else { self.next + 1 };
        return Ok(id);
      }
    }
    unreachable!()
  }

  /// 指定された ID のソケットを新規追加または更新します。
  pub fn set(&mut self, id: SocketId, socket: Socket) {
    self.sockets.insert(id, Arc::new(Mutex::new(socket)));
  }
}
