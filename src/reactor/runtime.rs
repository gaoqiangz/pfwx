#![allow(unused_imports)]

use std::{
    future::Future, panic, pin::Pin, sync::Mutex, thread::{self, JoinHandle}, time::Duration
};

use tokio::{
    runtime, sync::{
        mpsc::{self, UnboundedReceiver, UnboundedSender}, oneshot
    }, task
};

static GLOBAL_RUNTIME: Mutex<Option<Runtime>> = Mutex::new(None);

/// 在后台执行一个异步任务
#[cfg_attr(feature = "trace", track_caller)]
pub fn spawn<F>(fut: F)
where
    F: Future<Output = ()> + Send + 'static
{
    let mut runtime = GLOBAL_RUNTIME.lock().expect("Lock runtime failed");
    if runtime.is_none() {
        *runtime = Some(Runtime::new());
        #[cfg(feature = "trace")]
        debug!("Global runtime start");
    }
    let runtime_tx = runtime.as_ref().unwrap().msg_tx.as_ref().unwrap();
    #[cfg(feature = "trace")]
    let msg = Task(Box::pin(fut), panic::Location::caller());
    #[cfg(not(feature = "trace"))]
    let msg = Task(Box::pin(fut));
    if let Err(e) = runtime_tx.send(msg) {
        drop(runtime);
        panic!("Send message to runtime failed: {e:?}");
    }
}

/// 销毁后台运行时
pub fn shutdown() {
    let mut runtime = GLOBAL_RUNTIME.lock().expect("Lock runtime failed");
    if runtime.is_some() {
        *runtime = None;
        #[cfg(feature = "trace")]
        debug!("Global runtime shutdown");
    }
}

/// 异步任务
#[cfg(feature = "trace")]
struct Task(Pin<Box<dyn Future<Output = ()> + Send + 'static>>, &'static panic::Location<'static>);
#[cfg(not(feature = "trace"))]
struct Task(Pin<Box<dyn Future<Output = ()> + Send + 'static>>);

/// 运行时
struct Runtime {
    thrd_hdl: Option<JoinHandle<()>>,
    msg_tx: Option<UnboundedSender<Task>>,
    stop_rx: Option<oneshot::Receiver<()>>
}

impl Runtime {
    /// 创建运行时-支持日志调试
    #[cfg(feature = "trace")]
    fn new() -> Runtime {
        use std::{
            io::{Result as IoResult, Write}, str::from_utf8
        };

        use tracing::level_filters::LevelFilter;
        use tracing_subscriber::{filter, fmt, fmt::format::FmtSpan, prelude::*};
        use windows::{core::PCWSTR, Win32::System::Diagnostics::Debug::*};

        let filter = filter::Targets::default()
            .with_default(LevelFilter::OFF)
            .with_target(env!("CARGO_PKG_NAME"), LevelFilter::TRACE);

        // Log file
        let file_appender = tracing_appender::rolling::never("", concat!(env!("CARGO_PKG_NAME"), ".log"));
        let file = fmt::layer()
            .with_ansi(false)
            .with_span_events(FmtSpan::NONE)
            .with_line_number(true)
            .with_thread_names(true)
            .with_thread_ids(true)
            .with_writer(file_appender)
            .with_filter(filter.clone());
        // WinDBG
        struct OutputDebugString;
        impl Write for OutputDebugString {
            fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
                unsafe {
                    if let Ok(buf) = from_utf8(buf) {
                        let buf = widestring::U16CString::from_str_unchecked(buf);
                        OutputDebugStringW(PCWSTR::from_raw(buf.as_ptr()));
                    }
                }
                Ok(buf.len())
            }
            fn flush(&mut self) -> IoResult<()> { Ok(()) }
        }
        let dbg = fmt::layer()
            .with_ansi(false)
            .with_span_events(FmtSpan::NONE)
            .with_line_number(true)
            .with_thread_names(true)
            .with_thread_ids(true)
            .with_writer(|| OutputDebugString)
            .with_filter(filter.clone());
        // Console
        let (console, server) = console_subscriber::Builder::default().build();

        tracing_subscriber::registry().with(file).with(dbg).with(console).init();

        Self::startup_with_trace(server)
    }

    /// 创建运行时
    #[cfg(not(feature = "trace"))]
    fn new() -> Runtime { Self::startup_without_trace() }

    /// 启动运行时
    #[cfg(feature = "trace")]
    fn startup_with_trace(server: console_subscriber::Server) -> Runtime {
        Self::startup(|mut msg_rx| {
            async move {
                tokio::pin! {
                let server = server.serve();
                }
                loop {
                    tokio::select! {
                        msg = msg_rx.recv() => {
                            if let Some(Task(task, loc)) = msg {
                                task::Builder::new()
                                    .name(&format!("{}:{}", loc.file(), loc.line()))
                                    .spawn_local(task)
                                    .expect("Spawn local task failed");
                            } else {
                                break;
                            }
                        },
                        _ = &mut server => {}
                    }
                }
            }
        })
    }

    /// 启动运行时
    #[cfg(not(feature = "trace"))]
    fn startup_without_trace() -> Runtime {
        Self::startup(|mut msg_rx| {
            async move {
                while let Some(Task(task)) = msg_rx.recv().await {
                    task::spawn_local(task);
                }
            }
        })
    }

    /// 启动运行时
    fn startup<F, R>(runloop: F) -> Runtime
    where
        F: FnOnce(UnboundedReceiver<Task>) -> R,
        R: Future + Send + 'static
    {
        assert!(runtime::Handle::try_current().is_err());
        // 退出信号
        let (stop_tx, stop_rx) = oneshot::channel();
        // 消息通道
        let (msg_tx, msg_rx) = mpsc::unbounded_channel();
        let runloop = runloop(msg_rx);

        // 创建后台线程
        let thrd_hdl = thread::Builder::new()
            .name("bkgnd-rt".to_owned())
            .spawn(move || {
                // 单线程运行时
                let rt = runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("Create tokio runtime failed");
                let local = task::LocalSet::new();
                // 运行
                rt.block_on(local.run_until(runloop));
                rt.block_on(local);
                // NOTE
                // 运行时可能创建了`blocking`后台线程，此处需要立即退出并且不等待线程结束信号
                rt.shutdown_background();
                // 退出信号
                let _ = stop_tx.send(());
            })
            .expect("Create runtime thread failed");

        assert!(!msg_tx.is_closed());

        Runtime {
            thrd_hdl: Some(thrd_hdl),
            msg_tx: Some(msg_tx),
            stop_rx: Some(stop_rx)
        }
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        use std::os::windows::prelude::*;

        use windows::Win32::{
            Foundation::{HANDLE, WAIT_TIMEOUT}, System::Threading::WaitForSingleObject
        };

        // 关闭消息通道
        drop(self.msg_tx.take());

        // 检查线程是否存活，可能提前被`ExitProcess`销毁
        let thrd_hdl = self.thrd_hdl.take().unwrap();
        let rc = unsafe { WaitForSingleObject(HANDLE(thrd_hdl.as_raw_handle() as _), 0) };
        if rc == WAIT_TIMEOUT {
            // NOTE 不能直接WAIT线程对象，因为此时可能正处于TLS销毁流程中，OS加了保护锁防止不同线程同时进入`DllMain`
            // issue: https://github.com/rust-lang/rust/issues/74875
            let _ = self.stop_rx.take().unwrap().blocking_recv();
            // FIXME
            // 短暂挂起使线程调用栈完全退出
            thread::sleep(Duration::from_millis(200));
        }
    }
}
