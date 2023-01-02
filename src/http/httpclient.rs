use crate::prelude::*;
use pbni::{pbx::*, prelude::*};
use reactor::*;
use reqwest::{Client, ClientBuilder};
use std::{sync::Arc, time::Duration};
use tokio::sync::Mutex;

struct HttpClient {
    session: Session,
    ctx: ContextObject,
    state: HandlerState,
    seq_mtx: Arc<Mutex<()>>,
    client: Client
}

#[nonvisualobject(name = "nx_httpclient")]
impl HttpClient {
    #[constructor]
    fn new(session: Session, ctx: ContextObject) -> Self {
        let state = HandlerState::new();
        let seq_mtx = Arc::new(Mutex::new(()));
        let client = Client::new();
        HttpClient {
            session,
            ctx,
            state,
            seq_mtx,
            client
        }
    }

    fn context_mut(&mut self) -> &mut ContextObject { &mut self.ctx }

    #[method]
    fn config(&mut self, cfg: &mut HttpClientConfig) -> RetCode {
        if let Some(cfg) = cfg.builder.take() {
            self.client = cfg.build()?;
            RetCode::OK
        } else {
            RetCode::E_INVALID_ARGUMENT
        }
    }

    #[method]
    fn get(&self, url: String) -> String {
        let client = self.client.clone();
        self.spawn_blocking(async move {
            match client.get(url).send().await {
                Ok(resp) => resp.text().await.unwrap_or_default(),
                Err(e) => e.to_string()
            }
        })
        .unwrap()
    }

    #[method]
    fn get_with_event(&self, url: String, hevent: pbulong) -> String {
        let client = self.client.clone();
        self.spawn_blocking(async move {
            if let Some(rv) = futures::cancel_by_event(
                async move {
                    match client.get(url).send().await {
                        Ok(resp) => resp.text().await.unwrap_or_default(),
                        Err(e) => e.to_string()
                    }
                },
                hevent
            )
            .await
            {
                rv
            } else {
                "[cancelled]".to_string()
            }
        })
        .unwrap()
    }
}

impl Handler for HttpClient {
    fn session(&self) -> &Session { &self.session }
    fn state(&self) -> &HandlerState { &self.state }
}

struct HttpClientConfig {
    builder: Option<ClientBuilder>
}

#[nonvisualobject(name = "nx_httpconfig")]
impl HttpClientConfig {
    #[constructor]
    fn new(_: Session, _: ContextObject) -> Self {
        HttpClientConfig {
            builder: Some(ClientBuilder::default())
        }
    }

    #[method]
    fn set_agent(&mut self, val: String) {
        let builder = self.builder.take().unwrap().user_agent(val);
        self.builder.replace(builder);
    }

    #[method]
    fn set_timeout(&mut self, val: pbdouble) {
        let builder = self.builder.take().unwrap().timeout(Duration::from_secs_f64(val));
        self.builder.replace(builder);
    }
}
