use crate::prelude::*;
use pbni::{pbx::*, prelude::*};
use reactor::*;
use reqwest::{Client, Method};
use std::{
    collections::HashMap, fs, rc::Rc, sync::{Arc, Mutex as BlockingMutex}, thread
};
use tokio::sync::Mutex;

mod config;
mod response;
mod request;
mod form;
mod multipart;
mod cookie;

use config::{HttpClientConfig, HttpClientRuntimeConfig};
use request::HttpRequest;
use response::{HttpResponse, HttpResponseKind};

struct HttpClient {
    state: HandlerState,
    client: Client,
    cfg: Rc<HttpClientRuntimeConfig>,
    seq_lock: Arc<Mutex<()>>,
    pending: Rc<BlockingMutex<HashMap<pbulong, (CancelHandle, Option<String>)>>>
}

#[nonvisualobject(name = "nx_httpclient")]
impl HttpClient {
    #[constructor]
    fn new(session: Session, _object: Object) -> Self {
        let state = HandlerState::new(session);
        let client = Client::new();
        let cfg = Rc::new(HttpClientRuntimeConfig::default());
        let seq_lock = Arc::new(Mutex::new(()));
        let pending = Rc::new(BlockingMutex::new(HashMap::new()));
        HttpClient {
            state,
            client,
            cfg,
            seq_lock,
            pending
        }
    }

    fn push_pending(&self, id: pbulong, cancel_hdl: CancelHandle, receive_file: Option<String>) {
        let mut pending = self.pending.lock().unwrap();
        if let Some((hdl, receive_file)) = pending.insert(id, (cancel_hdl, receive_file)) {
            hdl.cancel();
            if let Some(file_path) = receive_file {
                thread::yield_now();
                let _ = fs::remove_file(file_path);
            }
        }
    }

    fn complete(&mut self, id: pbulong, resp: HttpResponseKind, elapsed: u128, receive_file: Option<String>) {
        let is_cancelled = resp.is_cancelled();
        let is_succ = resp.is_succ();
        let resp = HttpResponse::new_object_modify(self.get_session(), |obj| {
            obj.init(resp, elapsed, Some(id), receive_file)
        });
        let alive = self.get_alive_state();
        if !is_cancelled {
            if is_succ {
                self.on_succ(id, &resp);
            } else {
                self.on_error(id, &resp);
            }
        }
        //NOTE 对象可能被销毁
        if alive.is_alive() {
            self.on_complete(id, &resp);
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
    fn request(&mut self, method: String, url: String) -> Object {
        let method = match Method::from_str(&method.to_ascii_uppercase()) {
            Ok(method) => method,
            Err(_) => panic!("Unsupport method: {method}")
        };
        HttpRequest::new_object_modify(self.get_session(), |obj| {
            obj.init(self.get_object().share(), self.client.request(method, url));
        })
    }

    #[method(name = "Cancel")]
    fn cancel(&mut self, id: pbulong) -> RetCode {
        let mut pending = self.pending.lock().unwrap();
        if let Some((hdl, receive_file)) = pending.remove(&id) {
            drop(pending);
            if hdl.cancel() {
                self.complete(id, HttpResponseKind::cancelled(), 0, receive_file.clone());
            }
            if let Some(file_path) = receive_file {
                thread::yield_now();
                let _ = fs::remove_file(file_path);
            }
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
    fn state(&self) -> &HandlerState { &self.state }
    fn alive_state(&self) -> AliveState { self.get_alive_state() }
}
