use crate::{
    reactor::{Handler, HandlerState}, retcode
};
use pbni::{pbx::*, prelude::*};
use std::sync::Arc;
use tinyrand::{Rand, RandRange, StdRand};
use tokio::sync::Mutex;

struct HttpClient {
    session: Session,
    ctx: ContextObject,
    state: HandlerState,
    seq_mtx: Arc<Mutex<()>>,
    rand: StdRand,
    cnt: u32,
    inner: Option<AsyncTestInner>
}

#[nonvisualobject(name = "n_async_test")]
impl HttpClient {
    #[constructor]
    fn new(session: Session, ctx: ContextObject) -> Self {
        HttpClient {
            session,
            ctx,
            seq_mtx: Arc::new(Mutex::new(())),
            rand: StdRand::default(),
            cnt: 0,
            state: HandlerState::new(),
            inner: None
        }
    }

    #[method]
    fn version(&self) -> String { String::from("1.0") }

    #[method]
    fn copyright(&self) -> String { String::from(env!("CARGO_PKG_AUTHORS")) }

    #[method]
    fn async_call(&mut self) -> pblong {
        /*self.spawn_with_handler(
            async { Ok(reqwest::get("http://www.baidu.com").await?.text().await?) },
            |this, param: reqwest::Result<String>| {
                this.on_async(format!("{:?}", param)).unwrap();
            }
        )
        .cancel();

        let dispatcher = self.dispatcher();
        self.spawn_with_handler(
            async move {
                let mut cnt = 0;
                while cnt < 10 {
                    cnt += 1;
                    dispatcher
                        .dispatch_with_param(format!("tick {cnt}"), |this, param| {
                            this.on_async(param).unwrap();
                        })
                        .await;
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
                "tick done".to_owned()
            },
            |this, param| {
                this.on_async(param).unwrap();
            }
        );*/
        self.cnt += 1;
        let cnt = self.cnt;
        let sleep = self.rand.next_range(100..1000);
        let seq_mtx = self.seq_mtx.clone();
        self.spawn_with_handler(
            async move {
                let _seq_mtx = seq_mtx.lock().await;
                tokio::time::sleep(std::time::Duration::from_millis(sleep)).await;

                format!("{cnt}")
            },
            |this, param| {
                this.on_async(param).unwrap();
            }
        );
        retcode::OK
    }

    #[event(name = "onAsync")]
    fn on_async(&mut self, param: String) -> Result<()> {}
}

impl HttpClient {
    fn context_mut(&mut self) -> &mut ContextObject { &mut self.ctx }
}

impl Handler for HttpClient {
    fn session(&self) -> &Session { &self.session }
    fn state(&self) -> &HandlerState { &self.state }
}

#[allow(dead_code)]
struct AsyncTestInner {}
