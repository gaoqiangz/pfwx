//! 后台异步运行时服务
//!

use futures_util::FutureExt;
use pbni::pbx::Session;
use std::{
    future::Future, panic::{AssertUnwindSafe, UnwindSafe}, sync::{Arc, Weak}
};
use tokio::sync::oneshot;

mod context;
mod runtime;

use context::SyncContext;
use runtime::{Runtime, RuntimeMessage};

/// 可同步的对象抽象
pub trait SyncObject: UnwindSafe + 'static {
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
    /// `Cancellation`任务取消令牌
    fn spawn<F, H>(&mut self, fut: F, handler: H) -> Cancellation
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
        H: Fn(&mut Self, F::Output) + Send + UnwindSafe + 'static
    {
        self::spawn(self, fut, handler)
    }
}

/// 对象存活状态
#[derive(Default, Clone)]
pub struct AliveState(Arc<()>);

impl AliveState {
    pub fn new() -> AliveState { AliveState(Arc::new(())) }
    fn watch(&self) -> AliveWatch { AliveWatch(Arc::downgrade(&self.0)) }
}

/// 对象存活状态监视
struct AliveWatch(Weak<()>);

impl AliveWatch {
    fn is_alive(&self) -> bool { self.0.strong_count() != 0 }
    fn is_dead(&self) -> bool { self.0.strong_count() == 0 }
}

/// 启动一个异步任务
///
/// # Parameters
///
/// - `ctx` 回调处理对象传递给`handler`使用
/// - `fut` 异步任务
/// - `handler` 接收`fut`执行结果并在当前(UI)线程中执行
///
/// # Returns
///
/// `Cancellation`任务取消令牌
pub fn spawn<T, F, H>(ctx: &mut T, fut: F, handler: H) -> Cancellation
where
    T: SyncObject,
    F: Future + Send + 'static,
    F::Output: Send + 'static,
    H: Fn(&mut T, F::Output) + Send + UnwindSafe + 'static
{
    let sync_ctx = SyncContext::current(ctx.session());
    let runtime_tx = Runtime::global_sender();
    let (cancel_tx, cancel_rx) = oneshot::channel();
    //封装异步任务
    let task = {
        let handler = unsafe {
            let ctx = UnsafePointer::from_raw(ctx as *mut T);
            Box::new(move |param: UnsafeBox<()>, invoke: bool| {
                let param = param.cast::<F::Output>().unpack();
                if invoke {
                    let ctx = ctx.into_raw();
                    handler(&mut *ctx, param);
                }
            })
        };
        let alive = ctx.alive().watch();
        let dispatcher = sync_ctx.dispatcher();
        async move {
            let fut = AssertUnwindSafe(fut).catch_unwind();
            loop {
                tokio::select! {
                    rv = &mut fut => {
                        match rv {
                            Ok(rv) => {
                                let param = unsafe { UnsafeBox::pack(rv).cast::<()>() };
                                dispatcher.dispatch_invoke(param, handler, alive).await;
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
                    rv = &mut cancel_rx, if rv.is_ok() => break,
                }
            }
        }
    };
    if let Err(e) = runtime_tx.blocking_send(RuntimeMessage::Task(Box::pin(task))) {
        panic!("send message to background failed: {e}");
    }
    Cancellation(cancel_tx)
}

/// 异步任务取消令牌
pub struct Cancellation(oneshot::Sender<()>);

impl Cancellation {
    /// 取消异步任务
    pub fn cancel(self) { let _ = self.0.send(()); }
}

/// 非类型安全的堆分配器
#[repr(transparent)]
struct UnsafeBox<T>(*mut T);

#[allow(dead_code)]
impl<T> UnsafeBox<T> {
    unsafe fn from_raw(raw: *mut T) -> Self { UnsafePointer(raw) }
    unsafe fn into_raw(self) -> *mut T { self.0 }
    unsafe fn pack(rhs: T) -> Self { UnsafeBox(Box::into_raw(Box::new(rhs)) as _) }
    unsafe fn unpack(self) -> T { unsafe { Box::into_inner(Box::from_raw(self.0)) } }
    unsafe fn cast<U>(self) -> UnsafeBox<U> { UnsafeBox(self.0 as *mut U) }
    fn as_raw(&self) -> *mut T { self.0 }
}

unsafe impl<T: Send> Send for UnsafeBox<T> {}
impl<T: UnwindSafe> UnwindSafe for UnsafeBox<T> {}

/// 非线程安全的指针，使其可以在线程间传递
///
/// # Safety
///
/// **确保指针在归属线程中使用**
#[repr(transparent)]
struct UnsafePointer<T>(*mut T);

#[allow(dead_code)]
impl<T> UnsafePointer<T> {
    unsafe fn from_raw(raw: *mut T) -> Self { UnsafePointer(raw) }
    unsafe fn into_raw(self) -> *mut T { self.0 }
    unsafe fn cast<U>(self) -> UnsafePointer<U> { UnsafePointer(self.0 as *mut U) }
    fn as_raw(&self) -> *mut T { self.0 }
}

unsafe impl<T> Send for UnsafePointer<T> {}
impl<T: UnwindSafe> UnwindSafe for UnsafePointer<T> {}
