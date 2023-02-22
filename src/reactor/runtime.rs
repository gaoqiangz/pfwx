use std::{
    future::Future, pin::Pin, sync::{
        atomic::{AtomicUsize, Ordering}, Mutex
    }, thread
};
use tokio::{
    runtime, sync::{mpsc, oneshot}, task
};

static GLOBAL_RUNTIME: Mutex<Option<Runtime>> = Mutex::new(None);
thread_local! {
static COUNTER: Counter = Counter::new();
}
static COUNT: AtomicUsize = AtomicUsize::new(0);

/// 在后台执行一个异步任务
pub fn spawn<F>(fut: F)
where
    F: Future<Output = ()> + Send + 'static
{
    //初始化计数
    COUNTER.with(|_| ());
    //获取运行时
    let mut runtime = GLOBAL_RUNTIME.lock().unwrap();
    if runtime.is_none() {
        *runtime = Some(Runtime::new());
    }
    //推送任务
    let runtime_tx = runtime.as_ref().unwrap().msg_tx.clone();
    if let Err(e) = runtime_tx.send(RuntimeMessage::Task(Box::pin(fut))) {
        panic!("send message to background failed: {e}");
    }
}

/// 运行时消息
enum RuntimeMessage {
    Task(Pin<Box<dyn Future<Output = ()> + Send + 'static>>),
    Stop
}

/// 运行时
struct Runtime {
    msg_tx: mpsc::UnboundedSender<RuntimeMessage>,
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
                let runloop = async move {
                    while let Some(msg) = msg_rx.recv().await {
                        match msg {
                            RuntimeMessage::Task(task) => {
                                task::spawn_local(task);
                            },
                            RuntimeMessage::Stop => break
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
            msg_tx,
            stop_rx: Some(stop_rx)
        }
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        let _ = self.msg_tx.send(RuntimeMessage::Stop);
        //NOTE 不能直接WAIT线程对象，因为此时处于TLS销毁流程中，OS加了保护锁防止同时销毁
        self.stop_rx.take().unwrap().blocking_recv().unwrap();
        //FIXME
        //短暂挂起使线程调用栈完全退出
        thread::sleep(std::time::Duration::from_millis(200));
    }
}

struct Counter;

impl Counter {
    fn new() -> Counter {
        //计数
        COUNT.fetch_add(1, Ordering::Relaxed);
        Counter
    }
}

impl Drop for Counter {
    fn drop(&mut self) {
        if COUNT.fetch_sub(1, Ordering::Relaxed) == 1 {
            //FIXME
            //销毁运行时
            let mut runtime = GLOBAL_RUNTIME.lock().unwrap();
            *runtime = None;
        }
    }
}
