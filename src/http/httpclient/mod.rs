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

    #[method]
    fn reconfig(&mut self, cfg: &mut HttpClientConfig) -> RetCode {
        let (client, rt_cfg) = cfg.build()?;
        self.client = client;
        self.cfg = Rc::new(rt_cfg);
        RetCode::OK
    }

    #[method]
    fn get(&self, url: String) -> Result<Object> {
        HttpRequest::new_object_modify(&self.session, |obj| {
            obj.init(self.clone(), self.client.get(url));
        })
    }

    #[method]
    fn post(&self, url: String) -> Result<Object> {
        HttpRequest::new_object_modify(&self.session, |obj| {
            obj.init(self.clone(), self.client.post(url));
        })
    }

    #[method]
    fn put(&self, url: String) -> Result<Object> {
        HttpRequest::new_object_modify(&self.session, |obj| {
            obj.init(self.clone(), self.client.put(url));
        })
    }

    #[method]
    fn patch(&self, url: String) -> Result<Object> {
        HttpRequest::new_object_modify(&self.session, |obj| {
            obj.init(self.clone(), self.client.patch(url));
        })
    }

    #[method]
    fn delete(&self, url: String) -> Result<Object> {
        HttpRequest::new_object_modify(&self.session, |obj| {
            obj.init(self.clone(), self.client.delete(url));
        })
    }

    #[method]
    fn head(&self, url: String) -> Result<Object> {
        HttpRequest::new_object_modify(&self.session, |obj| {
            obj.init(self.clone(), self.client.head(url));
        })
    }

    #[method]
    fn request(&self, method: String, url: String) -> Result<Object> {
        let method = match Method::from_str(&method) {
            Ok(method) => method,
            Err(_) => return Err(PBXRESULT::E_INVALID_ARGUMENT)
        };
        HttpRequest::new_object_modify(&self.session, |obj| {
            obj.init(self.clone(), self.client.request(method, url));
        })
    }

    #[method]
    fn cancel(&self, id: pbulong) {
        let mut pending = self.pending.lock().unwrap();
        if let Some(hdl) = pending.remove(&id) {
            hdl.cancel();
        }
    }

    #[event(name = "OnReceive")]
    fn on_recv(&mut self, id: pbulong, total: pbulong, received: pbulong, speed: pbulong) {}
    #[event(name = "OnComplete")]
    fn on_complete(&mut self, id: pbulong, resp: Object) {}
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
