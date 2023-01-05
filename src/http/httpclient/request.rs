use super::{
    response::{HttpResponse, HttpResponseKind}, *
};
use bytes::BytesMut;
use futures_util::future::{self, Either};
use reqwest::{
    header::{HeaderName, HeaderValue}, RequestBuilder
};
use std::{future::Future, time::Duration};
use tokio::time::{self, Instant};

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

    pub(super) fn init(&mut self, client: SharedObject, builder: RequestBuilder) {
        self.inner = Some(HttpRequestInner {
            client,
            builder: Some(builder)
        });
    }

    #[method(name = "SetHeader")]
    fn header(&mut self, key: String, val: String) -> &ContextObject {
        if let Some(inner) = self.inner.as_mut() {
            let builder = inner.builder.take().unwrap();
            inner.builder.replace(builder.header(
                HeaderName::from_str(&key).expect("invalid header key"),
                HeaderValue::from_str(&val).expect("invalid header value")
            ));
        }
        &self.ctx
    }

    #[method(name = "SetBasicAuth")]
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
        }
        &self.ctx
    }

    #[method(name = "SetBearerAuth")]
    fn bearer_auth(&mut self, token: String) -> &ContextObject {
        if let Some(inner) = self.inner.as_mut() {
            let builder = inner.builder.take().unwrap();
            inner.builder.replace(builder.bearer_auth(token));
        }
        &self.ctx
    }

    #[method(name = "SetTimeout")]
    fn timeout(&mut self, secs: pbdouble) -> &ContextObject {
        if let Some(inner) = self.inner.as_mut() {
            let builder = inner.builder.take().unwrap();
            inner.builder.replace(builder.timeout(Duration::from_secs_f64(secs)));
        }
        &self.ctx
    }

    #[method(name = "Query")]
    fn query(&mut self, key: String, val: String) -> &ContextObject {
        if let Some(inner) = self.inner.as_mut() {
            let builder = inner.builder.take().unwrap();
            inner.builder.replace(builder.query(&[(key.as_str(), val.as_str())]));
        }
        &self.ctx
    }

    #[method(name = "Send", overload = 1)]
    fn send(&mut self, hevent: Option<pbulong>) -> Result<Object> {
        if let Some(HttpRequestInner {
            client,
            builder
        }) = self.inner.take()
        {
            let client = unsafe { client.get_native_ref::<HttpClient>().expect("httpclient invalid") };
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
                                    Err(e) => HttpResponseKind::receive_error(status, headers, e)
                                }
                            },
                            Err(e) => HttpResponseKind::send_error(e)
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
            HttpResponse::new_object_modify(&self.session, |obj| {
                obj.init(HttpResponseKind::send_error("invalid request object"))
            })
        }
    }

    #[method(name = "AsyncSend", overload = 1)]
    fn async_send(&mut self, id: pbulong, progress: Option<bool>) -> RetCode {
        if let Some(HttpRequestInner {
            client,
            builder
        }) = self.inner.take()
        {
            let client = unsafe { client.get_native_ref::<HttpClient>().expect("httpclient invalid") };
            //执行顺序锁
            let lock = if client.cfg.guarantee_order {
                Some(client.seq_lock.clone())
            } else {
                None
            };
            let fut = if progress.unwrap_or_default() {
                Either::Left(self.async_send_progress_impl(&client, builder.unwrap(), id))
            } else {
                Either::Right(self.async_send_no_progress_impl(builder.unwrap()))
            };
            let cancel_hdl = client.spawn(
                async move {
                    let _lock = if let Some(lock) = lock.as_ref() {
                        Some(lock.lock().await)
                    } else {
                        None
                    };
                    (id, fut.await)
                },
                |this, (id, resp)| {
                    let is_cancelled = resp.is_cancelled();
                    let is_succ = resp.is_succ();
                    let resp = HttpResponse::new_object_modify(&this.session, |obj| obj.init(resp)).unwrap();
                    if is_succ {
                        this.on_succ(id, &resp);
                    } else if !is_cancelled {
                        this.on_error(id, &resp);
                    }
                    this.on_complete(id, &resp);
                }
            );
            client.push_pending(id, cancel_hdl);
            RetCode::OK
        } else {
            RetCode::E_INVALID_OBJECT
        }
    }

    fn async_send_no_progress_impl(&self, builder: RequestBuilder) -> impl Future<Output = HttpResponseKind> {
        let sending = builder.send();
        async move {
            match sending.await {
                Ok(resp) => {
                    let status = resp.status();
                    let headers = resp.headers().clone();
                    match resp.bytes().await {
                        Ok(data) => HttpResponseKind::received(status, headers, data),
                        Err(e) => HttpResponseKind::receive_error(status, headers, e)
                    }
                },
                Err(e) => HttpResponseKind::send_error(e)
            }
        }
    }

    fn async_send_progress_impl(
        &self,
        client: &HttpClient,
        builder: RequestBuilder,
        id: pbulong
    ) -> impl Future<Output = HttpResponseKind> {
        let invoker = client.invoker();
        let sending = builder.send();
        async move {
            match sending.await {
                Ok(mut resp) => {
                    let status = resp.status();
                    let headers = resp.headers().clone();
                    let total_size = resp.content_length().unwrap_or_default();
                    let mut recv_size = 0;
                    let mut recv_data = BytesMut::with_capacity(total_size.max(256 * 1024) as usize);

                    //定时器（每秒计算一次速率并回调通知对象）
                    let mut tick_start = Instant::now();
                    let mut tick_interval =
                        time::interval_at(tick_start + Duration::from_secs(1), Duration::from_secs(1));
                    let mut tick_size = 0; //基准
                    let mut tick_invoke = Either::Left(future::pending());

                    loop {
                        tokio::select! {
                            chunk = resp.chunk() => {
                                match chunk {
                                    Ok(Some(chunk)) => {
                                        recv_size += chunk.len();
                                        recv_data.extend_from_slice(&chunk);
                                    },
                                    Ok(None) => break HttpResponseKind::received(status, headers, recv_data.freeze()),
                                    Err(e) => {
                                        break HttpResponseKind::receive_error(status, headers, e);
                                    }
                                }
                            },
                            _ = tick_interval.tick() => {
                                let speed = (recv_size - tick_size) as f32 / tick_start.elapsed().as_secs_f32();
                                tick_size = recv_size;
                                tick_start = Instant::now();
                                //UI线程阻塞时截流，丢弃中间的速率
                                if matches!(tick_invoke, Either::Left(_)) {
                                    tick_invoke = Either::Right(
                                        Box::pin(
                                            invoker.invoke(
                                                (id, total_size, recv_size, speed),
                                                |this, (id, total_size, recv_size, speed)| {
                                                    this.on_recv(id, total_size as pbulong, recv_size as pbulong, speed as pbulong)
                                                }
                                            )
                                        )
                                    );
                                }
                            },
                            rv = &mut tick_invoke => {
                                tick_invoke = Either::Left(future::pending());
                                match rv {
                                    Ok(rv) => {
                                        //取消
                                        if rv == RetCode::PREVENT {
                                            break HttpResponseKind::cancelled();
                                        }
                                    },
                                    Err(e) => panic!("callback panic: {e:?}")
                                }
                            }
                        }
                    }
                },
                Err(e) => HttpResponseKind::send_error(e)
            }
        }
    }
}

struct HttpRequestInner {
    client: SharedObject,
    builder: Option<RequestBuilder>
}
