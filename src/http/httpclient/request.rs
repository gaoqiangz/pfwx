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

#[derive(Default)]
pub struct HttpRequest {
    inner: Option<HttpRequestInner>
}

#[nonvisualobject(name = "nx_httprequest")]
impl HttpRequest {
    pub(super) fn init(&mut self, client: SharedObject, builder: RequestBuilder) {
        self.inner = Some(HttpRequestInner {
            client,
            builder: Some(builder)
        });
    }

    #[method(name = "SetHeader")]
    fn header(&mut self, key: String, val: String) -> &mut Self {
        if let Some(inner) = self.inner.as_mut() {
            let builder = inner.builder.take().unwrap();
            inner.builder.replace(builder.header(
                HeaderName::from_str(&key).expect("invalid header key"),
                HeaderValue::from_str(&val).expect("invalid header value")
            ));
        }
        self
    }

    #[method(name = "SetBasicAuth")]
    fn basic_auth(&mut self, user: String, psw: String) -> &mut Self {
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
        self
    }

    #[method(name = "SetBearerAuth")]
    fn bearer_auth(&mut self, token: String) -> &mut Self {
        if let Some(inner) = self.inner.as_mut() {
            let builder = inner.builder.take().unwrap();
            inner.builder.replace(builder.bearer_auth(token));
        }
        self
    }

    #[method(name = "SetTimeout")]
    fn timeout(&mut self, secs: pbdouble) -> &mut Self {
        if let Some(inner) = self.inner.as_mut() {
            let builder = inner.builder.take().unwrap();
            inner.builder.replace(builder.timeout(Duration::from_secs_f64(secs)));
        }
        self
    }

    #[method(name = "Query")]
    fn query(&mut self, key: String, val: String) -> &mut Self {
        if let Some(inner) = self.inner.as_mut() {
            let builder = inner.builder.take().unwrap();
            inner.builder.replace(builder.query(&[(key.as_str(), val.as_str())]));
        }
        self
    }

    #[method(name = "Send", overload = 1)]
    fn send(&mut self, hevent: Option<pbulong>) -> Result<Object> {
        if let Some(HttpRequestInner {
            client,
            builder
        }) = self.inner.take()
        {
            let client = client.get_native_ref::<HttpClient>().expect("httpclient invalid");
            let sending = builder.unwrap().send();
            let (resp, elapsed) = client
                .spawn_blocking(async move {
                    let inst = Instant::now();
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
                    let resp = if let Some(hevent) = hevent {
                        if let Some(rv) = futures::cancel_by_event(fut, hevent).await {
                            rv
                        } else {
                            HttpResponseKind::cancelled()
                        }
                    } else {
                        fut.await
                    };
                    (resp, inst.elapsed().as_millis())
                })
                .unwrap();
            HttpResponse::new_object_modify(self.get_session(), |obj| obj.init(resp, elapsed, None))
        } else {
            HttpResponse::new_object_modify(self.get_session(), |obj| {
                obj.init(HttpResponseKind::send_error("invalid request object"), 0, None)
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
            let client = client.get_native_ref::<HttpClient>().expect("httpclient invalid");
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
                    let inst = Instant::now();
                    let resp = fut.await;
                    (id, resp, inst.elapsed().as_millis())
                },
                |this, (id, resp, elapsed)| {
                    this.complete(id, resp, elapsed);
                }
            );
            client.push_pending(id, cancel_hdl);
            RetCode::OK
        } else {
            RetCode::E_INVALID_OBJECT
        }
    }

    /// 异步请求实现
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

    /// 带进度回调的异步请求实现
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

                    #[derive(PartialEq, Eq)]
                    enum DoneFlag {
                        Pending,
                        Invoke,
                        Invoking,
                        Done
                    }
                    let mut done_flag = DoneFlag::Pending;

                    loop {
                        tokio::select! {
                            chunk = resp.chunk() => {
                                match chunk {
                                    Ok(Some(chunk)) => {
                                        recv_size += chunk.len();
                                        recv_data.extend_from_slice(&chunk);
                                    },
                                    Ok(None) => {
                                        if done_flag == DoneFlag::Pending {
                                            done_flag = DoneFlag::Invoke;
                                        }
                                        if done_flag == DoneFlag::Invoke || done_flag == DoneFlag::Invoking{
                                            tokio::task::yield_now().await;
                                            continue;
                                        }
                                        break HttpResponseKind::received(status, headers, recv_data.freeze())
                                    },
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
                                    if done_flag == DoneFlag::Invoke {
                                        done_flag = DoneFlag::Invoking;
                                    }
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
                                    Err(InvokeError::TargetIsDead) => break HttpResponseKind::cancelled(),
                                    Err(InvokeError::Panic) => panic!("callback panic")
                                }
                                if done_flag == DoneFlag::Invoking {
                                    done_flag = DoneFlag::Done;
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
