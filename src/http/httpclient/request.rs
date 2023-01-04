use super::*;
use reqwest::{header, RequestBuilder};
use std::time::Duration;

pub struct HttpRequest {
    ctx: ContextObject,
    inner: Option<HttpRequestInner>
}

#[nonvisualobject(name = "nx_httprequest")]
impl HttpRequest {
    #[constructor]
    fn new(_session: Session, ctx: ContextObject) -> Self {
        HttpRequest {
            ctx,
            inner: None
        }
    }

    pub(super) fn init(&mut self, client: HttpClient, builder: RequestBuilder) {
        self.inner = Some(HttpRequestInner {
            client,
            builder: Some(builder)
        });
    }

    #[method]
    fn header(&mut self, key: String, val: String) -> &ContextObject {
        if let Some(inner) = self.inner.as_mut() {
            let builder = inner.builder.take().unwrap();
            inner.builder.replace(builder.header(
                header::HeaderName::from_str(&key).expect("invalid header key"),
                header::HeaderValue::from_str(&val).expect("invalid header value")
            ));
            &self.ctx
        } else {
            panic!("invalid object");
        }
    }

    #[method]
    fn basic_auth(&mut self, user: String, psw: String) -> &ContextObject {
        if let Some(inner) = self.inner.as_mut() {
            let builder = inner.builder.take().unwrap();
            inner.builder.replace(builder.basic_auth(
                user,
                if psw.is_empty() {
                    None
                } else {
                    Some(psw)
                }
            ));
            &self.ctx
        } else {
            panic!("invalid object");
        }
    }

    #[method]
    fn bearer_auth(&mut self, token: String) -> &ContextObject {
        if let Some(inner) = self.inner.as_mut() {
            let builder = inner.builder.take().unwrap();
            inner.builder.replace(builder.bearer_auth(token));
            &self.ctx
        } else {
            panic!("invalid object");
        }
    }

    #[method]
    fn timeout(&mut self, secs: pbdouble) -> &ContextObject {
        if let Some(inner) = self.inner.as_mut() {
            let builder = inner.builder.take().unwrap();
            inner.builder.replace(builder.timeout(Duration::from_secs_f64(secs)));
            &self.ctx
        } else {
            panic!("invalid object");
        }
    }

    #[method]
    fn query(&mut self, key: String, val: String) -> &ContextObject {
        if let Some(inner) = self.inner.as_mut() {
            let builder = inner.builder.take().unwrap();
            inner.builder.replace(builder.query(&[(key.as_str(), val.as_str())]));
            &self.ctx
        } else {
            panic!("invalid object");
        }
    }

    #[method]
    fn send(&mut self, hevent: Option<pbulong>) -> String {
        if let Some(HttpRequestInner {
            client,
            builder
        }) = self.inner.take()
        {
            let sending = builder.unwrap().send();
            client
                .spawn_blocking(async move {
                    if let Some(hevent) = hevent {
                        if let Some(rv) = futures::cancel_by_event(
                            async move {
                                match sending.await {
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
                    } else {
                        match sending.await {
                            Ok(resp) => resp.text().await.unwrap_or_default(),
                            Err(e) => e.to_string()
                        }
                    }
                })
                .unwrap()
        } else {
            panic!("invalid object");
        }
    }

    #[method]
    fn async_send(&mut self, id: pbulong) {
        if let Some(HttpRequestInner {
            client,
            builder
        }) = self.inner.take()
        {
            //执行顺序锁
            let lock = if client.cfg.guarantee_order {
                Some(client.seq_lock.clone())
            } else {
                None
            };
            let sending = builder.unwrap().send();
            let cancel_hdl = client.spawn(
                async move {
                    let _lock = if let Some(lock) = lock.as_ref() {
                        Some(lock.lock().await)
                    } else {
                        None
                    };
                    let rv = match sending.await {
                        Ok(resp) => resp.text().await.unwrap_or_default(),
                        Err(e) => e.to_string()
                    };
                    (id, rv)
                },
                HttpClient::on_complete
            );
            client.push_pending(id, cancel_hdl);
        } else {
            panic!("invalid object");
        }
    }
}

struct HttpRequestInner {
    client: HttpClient,
    builder: Option<RequestBuilder>
}
