use super::{
    response::{HttpResponse, HttpResponseKind}, *
};
use futures_util::StreamExt;
use reqwest::{
    header::{HeaderName, HeaderValue}, RequestBuilder
};
use std::time::Duration;

pub struct HttpRequest {
    session: Session,
    ctx: ContextObject,
    inner: Option<HttpRequestInner>
}

#[nonvisualobject(name = "nx_httprequest")]
impl HttpRequest {
    #[constructor]
    fn new(session: Session, ctx: ContextObject) -> Self {
        HttpRequest {
            session,
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
                HeaderName::from_str(&key).expect("invalid header key"),
                HeaderValue::from_str(&val).expect("invalid header value")
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

    #[method(overload = 1)]
    fn send(&mut self, hevent: Option<pbulong>) -> Result<Object> {
        if let Some(HttpRequestInner {
            client,
            builder
        }) = self.inner.take()
        {
            let sending = builder.unwrap().send();
            let resp = client
                .spawn_blocking(async move {
                    let fut = async move {
                        match sending.await {
                            Ok(resp) => {
                                let status = resp.status();
                                let headers = resp.headers().clone();
                                match resp.bytes().await {
                                    Ok(data) => HttpResponseKind::received(status, headers, data),
                                    Err(e) => HttpResponseKind::receive_error(status, headers, e.to_string())
                                }
                            },
                            Err(e) => HttpResponseKind::send_error(e.to_string())
                        }
                    };
                    if let Some(hevent) = hevent {
                        if let Some(rv) = futures::cancel_by_event(fut, hevent).await {
                            rv
                        } else {
                            HttpResponseKind::cancelled()
                        }
                    } else {
                        fut.await
                    }
                })
                .unwrap();
            HttpResponse::new_object_modify(&self.session, |obj| obj.init(resp))
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
            let invoker = client.invoker();
            let sending = builder.unwrap().send();
            /*let cancel_hdl = client.spawn(
                async move {
                    let _lock = if let Some(lock) = lock.as_ref() {
                        Some(lock.lock().await)
                    } else {
                        None
                    };
                    let rv = match sending.await {
                        Ok(resp) => {
                            let total_size = resp.content_length().unwrap_or_default() as _;
                            let mut recv_size = 0;
                            let mut stream = resp.bytes_stream();
                            while let Some(data) = stream.next().await {
                                let data = data.unwrap();
                                recv_size += data.len();
                                if invoker
                                    .invoke(
                                        (id, total_size, recv_size),
                                        |this, (id, total_size, recv_size)| {
                                            this.on_recv(id, total_size, received, speed);
                                        }
                                    )
                                    .await
                                    .is_err()
                                {
                                    bail!("操作被取消");
                                }
                            }
                            resp.text().await.unwrap_or_default()
                        },
                        Err(e) => e.to_string()
                    };
                    (id, rv)
                },
                |this, (id, rv)| {
                    this.on_complete(id, rv);
                }
            );
            client.push_pending(id, cancel_hdl);
            */
        } else {
            panic!("invalid object");
        }
    }
}

struct HttpRequestInner {
    client: HttpClient,
    builder: Option<RequestBuilder>
}
