use crate::prelude::*;
use pbni::{pbx::*, prelude::*};
use reactor::*;
use reqwest::{Client, Method};
use std::{
    collections::HashMap, rc::Rc, sync::{Arc, Mutex as BlockingMutex}
};
use tokio::sync::Mutex;

mod config;
mod response;
mod request;

use config::HttpClientConfig;

use self::{config::HttpClientRuntimeConfig, request::HttpRequest};

struct HttpClient {
    session: Session,
    ctx: ContextObject,
    state: HandlerState,
    client: Client,
    cfg: Rc<HttpClientRuntimeConfig>,
    seq_lock: Arc<Mutex<()>>,
    pending: Rc<BlockingMutex<HashMap<pbulong, CancelHandle>>>
}

#[nonvisualobject(name = "nx_httpclient")]
impl HttpClient {
    #[constructor]
    fn new(session: Session, ctx: ContextObject) -> Self {
        let state = HandlerState::new();
        let client = Client::new();
        let cfg = Rc::new(HttpClientRuntimeConfig::default());
        let seq_lock = Arc::new(Mutex::new(()));
        let pending = Rc::new(BlockingMutex::new(HashMap::new()));
        HttpClient {
            session,
            ctx,
            state,
            client,
            cfg,
            seq_lock,
            pending
        }
    }

    fn context_mut(&mut self) -> &mut ContextObject { &mut self.ctx }

    fn push_pending(&self, id: pbulong, cancel_hdl: CancelHandle) {
        let mut pending = self.pending.lock().unwrap();
        if let Some(hdl) = pending.insert(id, cancel_hdl) {
            hdl.cancel();
        }
    }

    #[method(name = "Reconfig")]
    fn reconfig(&mut self, cfg: &mut HttpClientConfig) -> RetCode {
        let (client, rt_cfg) = cfg.build()?;
        self.client = client;
        self.cfg = Rc::new(rt_cfg);
        RetCode::OK
    }

    #[method(name = "Request")]
    fn request(&self, method: String, url: String) -> Result<Object> {
        let method = match Method::from_str(&method.to_ascii_uppercase()) {
            Ok(method) => method,
            Err(_) => return Err(PBXRESULT::E_INVALID_ARGUMENT)
        };
        HttpRequest::new_object_modify(&self.session, |obj| {
            obj.init(self.ctx.share(), self.client.request(method, url));
        })
    }

    #[method(name = "Cancel")]
    fn cancel(&self, id: pbulong) -> RetCode {
        let mut pending = self.pending.lock().unwrap();
        if let Some(hdl) = pending.remove(&id) {
            hdl.cancel();
            RetCode::OK
        } else {
            RetCode::E_DATA_NOT_FOUND
        }
    }

    #[event(name = "OnSuccess")]
    fn on_succ(&mut self, id: pbulong, resp: &Object) {}

    #[event(name = "OnError")]
    fn on_error(&mut self, id: pbulong, resp: &Object) {}

    #[event(name = "OnComplete")]
    fn on_complete(&mut self, id: pbulong, resp: &Object) {}

    #[event(name = "OnReceive")]
    fn on_recv(&mut self, id: pbulong, total: pbulong, received: pbulong, speed: pbulong) -> RetCode {}
}

impl Handler for HttpClient {
    fn session(&self) -> &Session { &self.session }
    fn state(&self) -> &HandlerState { &self.state }
}
