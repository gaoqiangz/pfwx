use std::{future::Future, panic, pin::Pin, sync::Mutex, thread};
use tokio::{
    runtime, sync::{mpsc, oneshot}, task
};

static GLOBAL_RUNTIME: Mutex<Option<Runtime>> = Mutex::new(None);

/// 在后台执行一个异步任务
#[cfg_attr(feature = "trace", track_caller)]
pub fn spawn<F>(fut: F)
where
    F: Future<Output = ()> + Send + 'static
{
    let mut runtime = GLOBAL_RUNTIME.lock().unwrap();
    if runtime.is_none() {
        *runtime = Some(Runtime::new());
    }
    let runtime_tx = runtime.as_ref().unwrap().msg_tx.clone().unwrap();
    #[cfg(feature = "trace")]
    let msg = Task(Box::pin(fut), panic::Location::caller());
    #[cfg(not(feature = "trace"))]
    let msg = Task(Box::pin(fut));
    if let Err(e) = runtime_tx.send(msg) {
        panic!("send message to background failed: {e}");
    }
}

/// 销毁后台运行时
pub fn shutdown() {
    let mut runtime = GLOBAL_RUNTIME.lock().unwrap();
    *runtime = None;
}

/// 异步任务
#[cfg(feature = "trace")]
struct Task(Pin<Box<dyn Future<Output = ()> + Send + 'static>>, &'static panic::Location<'static>);
#[cfg(not(feature = "trace"))]
struct Task(Pin<Box<dyn Future<Output = ()> + Send + 'static>>);

/// 运行时
struct Runtime {
    msg_tx: Option<mpsc::UnboundedSender<Task>>,
    stop_rx: Option<oneshot::Receiver<()>>
}

impl Runtime {
    /// 创建运行时
    fn new() -> Runtime {
        assert!(runtime::Handle::try_current().is_err());
        //退出信号
        let (stop_tx, stop_rx) = oneshot::channel();
        //消息通道
        let (msg_tx, mut msg_rx) = mpsc::unbounded_channel();

        //创建后台线程
        thread::Builder::new()
            .name("bkgnd-rt".to_owned())
            .spawn(move || {
                let runloop = {
                    #[cfg(feature = "trace")]
                    {
                        use tracing_subscriber::prelude::*;
                        let (layer, server) = console_subscriber::Builder::default().build();
                        tracing_subscriber::registry().with(layer).init();
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
                                                .unwrap();
                                        } else {
                                            break;
                                        }
                                    },
                                    _ = &mut server => {}
                                }
                            }
                        }
                    }
                    #[cfg(not(feature = "trace"))]
                    async move {
                        while let Some(Task(task)) = msg_rx.recv().await {
                            task::spawn_local(task);
                        }
                    }
                };
                //单线程运行时
                let rt = runtime::Builder::new_current_thread().enable_all().build().unwrap();
                let local = task::LocalSet::new();
                //运行
                rt.block_on(local.run_until(runloop));
                rt.block_on(local);
                //NOTE
                //运行时可能创建了`blocking`后台线程，此处需要立即退出并且不等待线程结束信号
                rt.shutdown_background();
                //退出信号
                stop_tx.send(()).unwrap();
            })
            .expect("create bkgnd-rt thread");

        Runtime {
            msg_tx: Some(msg_tx),
            stop_rx: Some(stop_rx)
        }
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        self.msg_tx.take();
        //NOTE 不能直接WAIT线程对象，因为此时处于TLS销毁流程中，OS加了保护锁防止同时销毁
        self.stop_rx.take().unwrap().blocking_recv().unwrap();
        //FIXME
        //短暂挂起使线程调用栈完全退出
        thread::sleep(std::time::Duration::from_millis(200));
    }
}
