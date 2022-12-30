use crate::prelude::*;
use pbni::{pbx::*, prelude::*};
use reactor::*;
use std::sync::Arc;
use tokio::sync::Mutex;

struct HttpClient {
    session: Session,
    ctx: ContextObject,
    state: HandlerState,
    seq_mtx: Arc<Mutex<()>>,
    client: reqwest::Client,
    blocking_client: reqwest::blocking::Client
}

#[nonvisualobject(name = "nx_httpclient")]
impl HttpClient {
    #[constructor]
    fn new(session: Session, ctx: ContextObject) -> Self {
        let state = HandlerState::new();
        let seq_mtx = Arc::new(Mutex::new(()));
        let client = reqwest::Client::new();
        let blocking_client = reqwest::blocking::Client::new();
        HttpClient {
            session,
            ctx,
            state,
            seq_mtx,
            client,
            blocking_client
        }
    }

    fn context_mut(&mut self) -> &mut ContextObject { &mut self.ctx }
}

impl Handler for HttpClient {
    fn session(&self) -> &Session { &self.session }
    fn state(&self) -> &HandlerState { &self.state }
}

#[allow(dead_code)]
struct AsyncTestInner {}
