use super::{context::SyncContext, runtime, UnsafeBox, UnsafePointer};
use futures_util::FutureExt;
use pbni::pbx::Session;
use std::{
    future::Future, marker::PhantomData, panic::{AssertUnwindSafe, UnwindSafe}, sync::{Arc, Mutex, Weak}
};
use tokio::sync::oneshot;

/// 回调处理对象抽象
pub trait Handler: Sized + UnwindSafe + 'static {
    /// PB会话
    fn session(&self) -> &Session;

    /// 对象存活状态
    fn alive(&self) -> &AliveState;

    /// 启动一个异步任务
    ///
    /// # Parameters
    ///
    /// - `fut` 异步任务
    /// - `handler` 接收`fut`执行结果并在当前(UI)线程中执行
    ///
    /// # Returns
    ///
    /// `CancelHandle` 任务取消句柄
    ///
    /// # Cancellation
    ///
    /// - 通过`CancelHandle`自动取消
    /// - 此对象销毁时自动被取消
    fn spawn<F, H>(&mut self, fut: F, handler: H) -> CancelHandle
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
        H: Fn(&mut Self, F::Output) + Send + UnwindSafe + 'static
    {
        let sync_ctx = SyncContext::current(self.session());
        let dispatcher = sync_ctx.dispatcher();
        let alive = self.alive();
        let alive_watch = alive.watch();
        let (cancel_hdl, mut cancel_rx) = alive.new_cancel_handle();
        let handler = unsafe {
            let cancel_id = cancel_hdl.id();
            let this = UnsafePointer::from_raw(self);
            Box::new(move |param: UnsafeBox<()>, alive: AliveWatch| {
                let param = param.cast::<F::Output>().unpack();
                if alive.is_alive() {
                    let this = &mut *this.into_raw();
                    //删除取消ID成功说明任务没有被取消
                    if this.alive().remove_cancel_id(cancel_id) {
                        handler(this, param);
                    }
                }
            })
        };
        //封装异步任务
        let fut = async move {
            tokio::pin! {
            let fut = AssertUnwindSafe(fut).catch_unwind();
            }
            loop {
                tokio::select! {
                    rv = &mut fut => {
                        cancel_rx.close();
                        match rv {
                            Ok(rv) => {
                                //检查取消信号
                                if cancel_rx.try_recv().is_ok() {
                                    break;
                                }
                                //检查目标对象存活
                                if alive_watch.is_dead() {
                                    break;
                                }
                                let param = unsafe { UnsafeBox::pack(rv).cast::<()>() };
                                dispatcher.dispatch_invoke(param, handler, alive_watch).await;
                            },
                            Err(e) => {
                                let panic_info = match e.downcast_ref::<String>() {
                                    Some(e) => &e,
                                    None => {
                                        match e.downcast_ref::<&'static str>() {
                                            Some(e) => e,
                                            None => "unknown"
                                        }
                                    },
                                };
                                dispatcher
                                    .dispatch_panic(format!(
                                        "{}\r\nbacktrace:\r\n{:?}",
                                        panic_info,
                                        backtrace::Backtrace::new()
                                    ))
                                    .await;
                            }
                        }
                        break;
                    },
                    _ = &mut cancel_rx => break,
                }
            }
        };
        runtime::spawn(fut);
        cancel_hdl
    }
}

/// 对象存活状态
#[derive(Default)]
pub struct AliveState(Arc<Mutex<CancelManager>>);

impl AliveState {
    pub fn new() -> AliveState { AliveState(Arc::new(Mutex::new(CancelManager::default()))) }

    /// 存活状态监视
    fn watch(&self) -> AliveWatch { AliveWatch(Arc::downgrade(&self.0)) }

    /// 新建一个异步任务取消句柄
    fn new_cancel_handle(&self) -> (CancelHandle, oneshot::Receiver<()>) {
        let mut inner = self.0.lock().unwrap();
        let (id, rx) = inner.new_cancel_id();
        drop(inner);
        (
            CancelHandle {
                id,
                state: Arc::downgrade(&self.0),
                _marker: PhantomData
            },
            rx
        )
    }

    /// 通过取消ID删除取消通道
    fn remove_cancel_id(&self, id: u32) -> bool {
        let mut inner = self.0.lock().unwrap();
        inner.remove(id)
    }
}

/// 对象存活状态监视
pub struct AliveWatch(Weak<Mutex<CancelManager>>);

impl AliveWatch {
    /// 是否存活
    pub fn is_alive(&self) -> bool { self.0.strong_count() != 0 }

    /// 是否死亡
    pub fn is_dead(&self) -> bool { self.0.strong_count() == 0 }
}

/// 异步任务取消管理器
#[derive(Default)]
struct CancelManager {
    next_id: u32,
    pending: Vec<(u32, oneshot::Sender<()>)>
}

impl CancelManager {
    /// 新建取消ID
    fn new_cancel_id(&mut self) -> (u32, oneshot::Receiver<()>) {
        let id = self.next_id;
        self.next_id += 1;
        let (tx, rx) = oneshot::channel();
        //先查找失效的Sender(任务Panic后不会删除)
        if let Some(idx) = self.pending.iter().position(|(_, tx)| tx.is_closed()) {
            self.pending[idx] = (id, tx);
        } else {
            self.pending.push((id, tx));
        }
        (id, rx)
    }

    /// 取消任务
    fn cancel(&mut self, id: u32) {
        if let Some(idx) = self.pending.iter().position(|item| item.0 == id) {
            let (_, tx) = self.pending.remove(idx);
            let _ = tx.send(());
        }
    }

    /// 删除取消通道
    fn remove(&mut self, id: u32) -> bool {
        let len = self.pending.len();
        self.pending.retain(|item| item.0 != id);
        len != self.pending.len()
    }
}

impl Drop for CancelManager {
    fn drop(&mut self) {
        //取消所有未完成的任务
        while let Some((_, tx)) = self.pending.pop() {
            let _ = tx.send(());
        }
    }
}

/// 异步任务取消句柄
#[derive(Clone)]
pub struct CancelHandle {
    id: u32,
    state: Weak<Mutex<CancelManager>>,
    // !Send
    _marker: PhantomData<*mut ()>
}

impl CancelHandle {
    /// 取消异步任务
    pub fn cancel(self) {
        if let Some(state) = self.state.upgrade() {
            let mut state = state.lock().unwrap();
            state.cancel(self.id);
        }
    }

    fn id(&self) -> u32 { self.id }
}
