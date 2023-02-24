use crate::prelude::*;
use pbni::{pbx::*, prelude::*};
use reactor::*;
use reqwest::{Client, Method};
use std::{cell::RefCell, collections::HashMap, fs, mem, rc::Rc, sync::Arc, thread};
use tokio::sync::Semaphore;

mod config;
mod response;
mod request;
mod form;
mod multipart;
mod cookie;

use config::HttpClientConfig;
use request::HttpRequest;
use response::{HttpResponse, HttpResponseKind};

struct HttpClient {
    state: HandlerState,
    client: Client,
    semaphore: Arc<Semaphore>,
    pending: Rc<RefCell<HashMap<pbulong, (CancelHandle, Option<String>)>>>
}

#[nonvisualobject(name = "nx_httpclient")]
impl HttpClient {
    #[constructor]
    fn new(session: Session, _object: Object) -> Self {
        let state = HandlerState::new(session);
        let client = Client::new();
        let semaphore = Arc::new(Semaphore::new(256));
        let pending = Rc::new(RefCell::new(HashMap::new()));
        HttpClient {
            state,
            client,
            semaphore,
            pending
        }
    }

    fn push_pending(&self, id: pbulong, cancel_hdl: CancelHandle, receive_file: Option<String>) {
        let mut pending = self.pending.borrow_mut();
        let old = pending.insert(id, (cancel_hdl, receive_file));
        drop(pending);
        if let Some((hdl, receive_file)) = old {
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
        let (client, cfg) = cfg.build()?;
        self.client = client;
        self.semaphore = Arc::new(Semaphore::new(cfg.max_concurrency.max(1)));
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
        let mut pending = self.pending.borrow_mut();
        let removed = pending.remove(&id);
        drop(pending);
        if let Some((hdl, receive_file)) = removed {
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

    #[method(name = "CancelAll")]
    fn cancel_all(&mut self) -> RetCode {
        let mut pending = self.pending.borrow_mut();
        let taked = mem::take(&mut *pending);
        drop(pending);
        for (id, (hdl, receive_file)) in taked {
            if hdl.cancel() {
                self.complete(id, HttpResponseKind::cancelled(), 0, receive_file.clone());
            }
            if let Some(file_path) = receive_file {
                thread::yield_now();
                let _ = fs::remove_file(file_path);
            }
        }
        RetCode::OK
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

impl Drop for HttpClient {
    fn drop(&mut self) {
        let mut pending = self.pending.borrow_mut();
        let taked = mem::take(&mut *pending);
        drop(pending);
        for (_, (hdl, receive_file)) in taked {
            hdl.cancel();
            if let Some(file_path) = receive_file {
                thread::yield_now();
                let _ = fs::remove_file(file_path);
            }
        }
    }
}
