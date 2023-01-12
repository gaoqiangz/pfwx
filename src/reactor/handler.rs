use super::{
    context::{Dispatcher, SyncContext}, runtime, UnsafeBox, UnsafePointer
};
use futures_util::FutureExt;
use pbni::pbx::{AliveState, Session};
use std::{
    cell::RefCell, future::Future, marker::PhantomData, panic::AssertUnwindSafe, rc::{Rc, Weak}, thread, thread::ThreadId
};
use tokio::sync::oneshot;

/// 回调处理对象抽象
pub trait Handler: Sized + 'static {
    /// 对象状态
    fn state(&self) -> &HandlerState;

    /// 存活状态
    fn alive_state(&self) -> AliveState;

    /// 对象回调派发器
    fn invoker(&self) -> HandlerInvoker<Self> { HandlerInvoker::bind(self) }

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
    /// - 通过`CancelHandle`手动取消
    /// - 此对象销毁时自动取消
    fn spawn<F, H>(&self, fut: F, handler: H) -> CancelHandle
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
        H: FnOnce(&mut Self, F::Output) + Send + 'static
    {
        let invoker = self.invoker();
        let (cancel_hdl, mut cancel_rx) = self.state().new_cancel_handle();
        let handler = {
            let cancel_id = cancel_hdl.id();
            move |this: &mut Self, param: F::Output| {
                //删除取消ID成功说明任务没有被取消
                if this.state().remove_cancel_id(cancel_id) {
                    handler(this, param);
                }
            }
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
                                let _ = invoker.invoke(rv, handler).await;
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
                                invoker
                                    .panic(panic_info)
                                    .await;
                            }
                        }
                        break;
                    },
                    _ = &mut cancel_rx => break,
                }
            }
        };

        //执行
        runtime::spawn(fut);

        cancel_hdl
    }

    /// 阻塞启动一个异步任务
    ///
    /// # Parameters
    ///
    /// - `fut` 异步任务
    ///
    /// # Deadlock
    ///
    /// 在`fut`内部请求UI回调将会发生死锁
    ///
    /// # Returns
    ///
    /// `fut` 任务的执行结果
    fn blocking_spawn<F, R>(&self, fut: F) -> Result<R, SpawnBlockingError>
    where
        F: Future<Output = R> + Send + 'static,
        R: Send + 'static
    {
        let (tx, rx) = oneshot::channel();
        //封装异步任务
        let fut = async move {
            match AssertUnwindSafe(fut).catch_unwind().await {
                Ok(rv) => assert!(tx.send(Ok(rv)).is_ok()),
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
                    assert!(tx.send(Err(SpawnBlockingError::Panic(panic_info.to_owned()))).is_ok());
                }
            }
        };
        //执行
        runtime::spawn(fut);
        //阻塞等待执行结果
        rx.blocking_recv().unwrap()
    }
}

/// 阻塞任务错误
#[derive(Debug, thiserror::Error)]
pub enum SpawnBlockingError {
    #[error("panic: {0}")]
    Panic(String)
}

/// 回调处理对象的状态
pub struct HandlerState {
    session: Session,
    mgr: Rc<RefCell<HandlerStateManager>>
}

impl HandlerState {
    pub fn new(session: Session) -> Self {
        HandlerState {
            session,
            mgr: Default::default()
        }
    }

    /// PB会话
    fn session(&self) -> &Session { &self.session }

    /// 新建一个异步任务取消句柄
    fn new_cancel_handle(&self) -> (CancelHandle, oneshot::Receiver<()>) {
        let mut mgr = self.mgr.borrow_mut();
        let (id, rx) = mgr.new_cancel_id();
        drop(mgr);
        (
            CancelHandle {
                id,
                mgr: Rc::downgrade(&self.mgr),
                _marker: PhantomData
            },
            rx
        )
    }

    /// 通过取消ID删除取消句柄
    fn remove_cancel_id(&self, id: u64) -> bool {
        let mut mgr = self.mgr.borrow_mut();
        mgr.remove_cancel(id)
    }
}

/// 异步任务状态管理器
#[derive(Default)]
struct HandlerStateManager {
    next_id: u64,
    pending: Vec<(u64, oneshot::Sender<()>)>
}

impl HandlerStateManager {
    /// 新建取消ID
    fn new_cancel_id(&mut self) -> (u64, oneshot::Receiver<()>) {
        let id = self.next_id;
        self.next_id += 1;
        let (tx, rx) = oneshot::channel();
        //优先覆盖失效的元素(任务Panic后残留)
        if let Some(idx) = self.pending.iter().position(|(_, tx)| tx.is_closed()) {
            self.pending[idx] = (id, tx);
        } else {
            self.pending.push((id, tx));
        }
        (id, rx)
    }

    /// 取消任务
    fn cancel(&mut self, id: u64) -> bool {
        if let Some(idx) = self.pending.iter().position(|item| item.0 == id) {
            let (_, tx) = self.pending.remove(idx);
            let _ = tx.send(());
            true
        } else {
            false
        }
    }

    /// 删除取消通道
    fn remove_cancel(&mut self, id: u64) -> bool {
        let len = self.pending.len();
        self.pending.retain(|item| item.0 != id);
        len != self.pending.len()
    }
}

impl Drop for HandlerStateManager {
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
    id: u64,
    mgr: Weak<RefCell<HandlerStateManager>>,
    // !Send
    _marker: PhantomData<*mut ()>
}

impl CancelHandle {
    /// 取消异步任务
    pub fn cancel(self) -> bool {
        if let Some(mgr) = self.mgr.upgrade() {
            let mut mgr = mgr.borrow_mut();
            mgr.cancel(self.id)
        } else {
            false
        }
    }

    /// 取消ID
    fn id(&self) -> u64 { self.id }
}

/// 对象回调派发器
pub struct HandlerInvoker<T> {
    this: UnsafePointer<T>,
    alive: AliveState,
    dsp: Dispatcher,
    thread_id: ThreadId
}

impl<T: Handler> HandlerInvoker<T> {
    /// 创建派发器并绑定对象
    fn bind(this: &T) -> Self {
        let sync_ctx = SyncContext::current(this.state().session());
        HandlerInvoker {
            this: unsafe { UnsafePointer::from_raw(this as *const T as *mut T) },
            alive: this.alive_state(),
            dsp: sync_ctx.dispatcher(),
            thread_id: thread::current().id()
        }
    }

    /// 发起回调请求给UI线程执行
    ///
    /// # Parameters
    ///
    /// - `param` 参数
    /// - `handler` 接收`param`参数的回调过程并在UI线程中执行
    ///
    /// # Returns
    ///
    /// 成功返回`handler`返回值
    pub async fn invoke<P, H, R>(&self, param: P, handler: H) -> Result<R, InvokeError>
    where
        P: Send + 'static,
        H: FnOnce(&mut T, P) -> R + Send + 'static,
        R: Send + 'static
    {
        assert_ne!(self.thread_id, thread::current().id());
        if self.alive.is_dead() {
            return Err(InvokeError::TargetIsDead);
        }
        let (tx, rx) = oneshot::channel();
        let handler = unsafe {
            let this = self.this.clone();
            Box::new(move |param: UnsafeBox<()>, invoke: bool| {
                let param = param.cast::<P>().unpack();
                let rv = if invoke {
                    let this = &mut *this.into_raw();
                    Some(handler(this, param))
                } else {
                    None
                };
                //异步任务可能被取消
                let _ = tx.send(rv);
            })
        };
        let param = UnsafeBox::pack(param).cast::<()>();
        if !self.dsp.dispatch_invoke(param, handler, self.alive.clone()).await {
            return Err(InvokeError::TargetIsDead);
        }
        match rx.await {
            Ok(Some(rv)) => Ok(rv),
            Ok(None) => Err(InvokeError::TargetIsDead),
            Err(_) => {
                //回调过程发生异常导致`tx`被提前销毁
                Err(InvokeError::Panic)
            }
        }
    }

    /// 阻塞发起回调请求给UI线程执行
    ///
    /// # Description
    ///
    /// 在非异步上下文中使用
    ///
    /// # Parameters
    ///
    /// - `param` 参数
    /// - `handler` 接收`param`参数的回调过程并在UI线程中执行
    ///
    /// # Returns
    ///
    /// 成功返回`handler`返回值
    pub fn blocking_invoke<P, H, R>(&self, param: P, handler: H) -> Result<R, InvokeError>
    where
        P: Send + 'static,
        H: FnOnce(&mut T, P) -> R + Send + 'static,
        R: Send + 'static
    {
        assert_ne!(self.thread_id, thread::current().id());
        if self.alive.is_dead() {
            return Err(InvokeError::TargetIsDead);
        }
        let (tx, rx) = oneshot::channel();
        let handler = unsafe {
            let this = self.this.clone();
            Box::new(move |param: UnsafeBox<()>, invoke: bool| {
                let param = param.cast::<P>().unpack();
                let rv = if invoke {
                    let this = &mut *this.into_raw();
                    Some(handler(this, param))
                } else {
                    None
                };
                //异步任务可能被取消
                let _ = tx.send(rv);
            })
        };
        let param = UnsafeBox::pack(param).cast::<()>();
        if !self.dsp.blocking_dispatch_invoke(param, handler, self.alive.clone()) {
            return Err(InvokeError::TargetIsDead);
        }
        match rx.blocking_recv() {
            Ok(Some(rv)) => Ok(rv),
            Ok(None) => Err(InvokeError::TargetIsDead),
            Err(_) => {
                //回调过程发生异常导致`tx`被提前销毁
                Err(InvokeError::Panic)
            }
        }
    }

    /// 派发执行异常信息给UI线程
    async fn panic(&self, panic_info: &str) -> bool {
        self.dsp
            .dispatch_panic(format!("{}\r\nbacktrace:\r\n{:?}", panic_info, backtrace::Backtrace::new()))
            .await
    }

    /// 阻塞派发执行异常信息给UI线程
    ///
    /// # Description
    ///
    /// 在非异步上下文中使用
    fn blocking_panic(&self, panic_info: &str) -> bool {
        self.dsp.blocking_dispatch_panic(format!(
            "{}\r\nbacktrace:\r\n{:?}",
            panic_info,
            backtrace::Backtrace::new()
        ))
    }
}

impl<T> Clone for HandlerInvoker<T> {
    fn clone(&self) -> Self {
        HandlerInvoker {
            this: self.this.clone(),
            alive: self.alive.clone(),
            dsp: self.dsp.clone(),
            thread_id: self.thread_id.clone()
        }
    }
}

/// UI线程调用错误
#[derive(Debug, thiserror::Error)]
pub enum InvokeError {
    #[error("target is dead")]
    TargetIsDead,
    #[error("panic")]
    Panic
}
