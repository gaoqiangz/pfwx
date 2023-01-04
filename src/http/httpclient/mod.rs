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

    #[method]
    fn reconfig(&mut self, cfg: &mut HttpClientConfig) -> RetCode {
        let (client, rt_cfg) = cfg.build()?;
        self.client = client;
        self.cfg = Rc::new(rt_cfg);
        RetCode::OK
    }

    #[method]
    fn request(&self, method: String, url: String) -> Result<Object> {
        let method = match Method::from_str(&method) {
            Ok(method) => method,
            Err(_) => return Err(PBXRESULT::E_INVALID_ARGUMENT)
        };
        let mut obj = self.session.new_user_object(HttpRequest::CLASS_NAME)?;
        {
            let obj = unsafe { obj.get_native_mut::<HttpRequest>()? };
            obj.init(self.clone(), self.client.request(method, url));
        }
        Ok(obj)
    }

    #[method]
    fn cancel(&self, id: pbulong) {
        let mut pending = self.pending.lock().unwrap();
        if let Some(hdl) = pending.remove(&id) {
            hdl.cancel();
        }
    }

    fn push_pending(&self, id: pbulong, cancel_hdl: CancelHandle) {
        let mut pending = self.pending.lock().unwrap();
        if let Some(hdl) = pending.insert(id, cancel_hdl) {
            hdl.cancel();
        }
    }

    fn on_complete(&mut self, (id, resp): (pbulong, String)) {}
}

impl Handler for HttpClient {
    fn session(&self) -> &Session { &self.session }
    fn state(&self) -> &HandlerState { &self.state }
}

impl Clone for HttpClient {
    fn clone(&self) -> Self {
        HttpClient {
            session: unsafe { self.session.clone() },
            ctx: unsafe { self.ctx.clone() },
            state: self.state.clone(),
            client: self.client.clone(),
            cfg: self.cfg.clone(),
            seq_lock: self.seq_lock.clone(),
            pending: self.pending.clone()
        }
    }
}
