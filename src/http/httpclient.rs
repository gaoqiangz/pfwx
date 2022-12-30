use crate::{
    reactor::{Handler, HandlerState}, retcode
};
use pbni::{pbx::*, prelude::*};

struct AsyncTest {
    session: Session,
    ctx: ContextObject,
    state: HandlerState,
    inner: Option<AsyncTestInner>
}

#[nonvisualobject(name = "n_async_test")]
impl AsyncTest {
    #[constructor]
    fn new(session: Session, ctx: ContextObject) -> Self {
        AsyncTest {
            session,
            ctx,
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
        self.spawn_with_handler(
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
        );

        retcode::OK
    }

    #[event(name = "onAsync")]
    fn on_async(&mut self, param: String) -> Result<()> {}
}

impl AsyncTest {
    fn context_mut(&mut self) -> &mut ContextObject { &mut self.ctx }
}

impl Handler for AsyncTest {
    fn session(&self) -> &Session { &self.session }
    fn state(&self) -> &HandlerState { &self.state }
}

#[allow(dead_code)]
struct AsyncTestInner {}
