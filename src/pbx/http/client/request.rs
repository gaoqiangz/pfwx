use super::{form::HttpForm, multipart::HttpMultipart, *};
use crate::base::pfw;
use bytes::Bytes;
use futures_util::{
    future::{self, Either, FutureExt}, Stream
};
use http_body::Body as HttpBody;
use reqwest::{
    header::{self, HeaderValue, CONTENT_LENGTH}, Body, RequestBuilder, Response, Result as ReqwestResult
};
use std::{
    future::Future, pin::Pin, result::Result as StdResult, sync::atomic::{AtomicU64, Ordering}, task::{ready, Context as TaskContext, Poll}, time::Duration
};
use tokio::time::{self, Instant};

#[derive(Default)]
pub struct HttpRequest {
    inner: Option<HttpRequestInner>,
    recv_file_path: Option<String>
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
            inner.builder.replace(builder.header(key, val));
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

    #[method(name = "SetBody", overload = 1)]
    fn text(&mut self, text: String, content_type: Option<String>) -> &mut Self {
        if let Some(inner) = self.inner.as_mut() {
            let builder = inner.builder.take().unwrap();
            let mut builder = builder.body(text);
            builder = builder.header(
                header::CONTENT_TYPE,
                content_type.unwrap_or_else(|| mime::TEXT_PLAIN_UTF_8.to_string())
            );
            inner.builder.replace(builder);
        }
        self
    }

    #[method(name = "SetBody", overload = 1)]
    fn binary(&mut self, data: &[u8], content_type: Option<String>) -> &mut Self {
        if let Some(inner) = self.inner.as_mut() {
            let builder = inner.builder.take().unwrap();
            let mut builder = builder.body(data.to_owned());
            builder = builder.header(
                header::CONTENT_TYPE,
                content_type.unwrap_or_else(|| mime::APPLICATION_OCTET_STREAM.to_string())
            );
            inner.builder.replace(builder);
        }
        self
    }

    #[method(name = "SetBody")]
    fn json_or_xml(&mut self, obj: Object) -> &mut Self {
        if let Some(inner) = self.inner.as_mut() {
            let (data, content_type) = match obj.get_class_name().as_str() {
                "n_json" => (pfw::json_serialize(&obj), "application/json; charset=utf-8"),
                "n_xmldoc" => (pfw::xml_serialize(&obj), "text/xml; charset=utf-8"),
                cls @ _ => panic!("unexpect class {cls}")
            };
            let builder = inner.builder.take().unwrap();
            let mut builder = builder.body(data);
            builder = builder.header(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
            inner.builder.replace(builder);
        }
        self
    }

    #[method(name = "SetBody")]
    fn multipart(&mut self, form: &mut HttpMultipart) -> &mut Self {
        if let Some(inner) = self.inner.as_mut() {
            let builder = inner.builder.take().unwrap();
            inner.builder.replace(builder.multipart(form.build()));
        }
        self
    }

    #[method(name = "SetBody")]
    fn form(&mut self, form: &mut HttpForm) -> &mut Self {
        if let Some(inner) = self.inner.as_mut() {
            let builder = inner.builder.take().unwrap();
            inner.builder.replace(builder.form(&form.build()));
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

    #[method(name = "SetReceiveFile")]
    fn receive_file(&mut self, file_path: String) -> &mut Self {
        self.recv_file_path = Some(file_path);
        self
    }

    #[method(name = "Send", overload = 2)]
    fn send(&mut self, hevent: Option<pbulong>, progress: Option<bool>) -> Object {
        if let Some(HttpRequestInner {
            client,
            builder
        }) = self.inner.take()
        {
            let client = client.get_native_ref::<HttpClient>().expect("invalid httpclient");
            let recv_file_path = self.recv_file_path.clone();
            let fut = if progress.unwrap_or_default() {
                Either::Left(self.send_with_progress_impl(
                    0,
                    &client,
                    builder.unwrap(),
                    recv_file_path.clone()
                ))
            } else {
                Either::Right(self.send_impl(builder.unwrap(), recv_file_path.clone()))
            };
            let (resp, elapsed) = client
                .spawn_blocking(async move {
                    let inst = Instant::now();
                    let hevent = hevent.unwrap_or_default();
                    let resp = if hevent != 0 {
                        if let Some(rv) = futures::cancel_by_event(fut, hevent).await {
                            rv
                        } else {
                            HttpResponseInner::cancelled()
                        }
                    } else {
                        fut.await
                    };
                    (resp, inst.elapsed().as_millis())
                })
                .unwrap();
            HttpResponse::new_object_modify(self.get_session(), |obj| {
                obj.init(resp, elapsed, None, self.recv_file_path.take())
            })
        } else {
            HttpResponse::new_object_modify(self.get_session(), |obj| {
                obj.init(
                    HttpResponseInner::send_error("invalid request object"),
                    0,
                    None,
                    self.recv_file_path.take()
                )
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
            let client = client.get_native_ref::<HttpClient>().expect("invalid httpclient");
            let recv_file_path = self.recv_file_path.clone();
            //执行顺序锁
            let semaphore = client.semaphore.clone();
            let fut = if progress.unwrap_or_default() {
                Either::Left(self.send_with_progress_impl(
                    id,
                    &client,
                    builder.unwrap(),
                    recv_file_path.clone()
                ))
            } else {
                Either::Right(self.send_impl(builder.unwrap(), recv_file_path.clone()))
            };
            let cancel_hdl = client.spawn(
                async move {
                    let _permit = semaphore.acquire().await;
                    let inst = Instant::now();
                    let resp = fut.await;
                    (id, resp, inst.elapsed().as_millis())
                },
                move |this, (id, resp, elapsed)| {
                    this.complete(id, resp, elapsed, recv_file_path);
                }
            );
            client.push_pending(id, cancel_hdl, self.recv_file_path.take());
            RetCode::OK
        } else {
            RetCode::E_INVALID_OBJECT
        }
    }

    /// 请求实现
    fn send_impl(
        &mut self,
        builder: RequestBuilder,
        recv_file_path: Option<String>
    ) -> impl Future<Output = HttpResponseInner> {
        async move {
            match builder.send().await {
                Ok(resp) => HttpResponseInner::receive(resp, recv_file_path).await,
                Err(e) => HttpResponseInner::send_error(e)
            }
        }
    }

    /// 带进度回调的请求实现
    fn send_with_progress_impl(
        &mut self,
        id: pbulong,
        client: &HttpClient,
        builder: RequestBuilder,
        recv_file_path: Option<String>
    ) -> impl Future<Output = HttpResponseInner> {
        let invoker = client.invoker();
        async move {
            match Self::execute_request_with_progress(id, builder, invoker.clone()).await {
                Ok(resp) => HttpResponseInner::receive_with_progress(id, invoker, resp, recv_file_path).await,
                Err(e) => e
            }
        }
    }

    /// 执行带进度回调的请求
    async fn execute_request_with_progress(
        id: pbulong,
        builder: RequestBuilder,
        invoker: HandlerInvoker<HttpClient>
    ) -> StdResult<Response, HttpResponseInner> {
        let (raw_client, mut req) = match builder.build_split() {
            (cli, Ok(req)) => (cli, req),
            (_, Err(e)) => return Err(HttpResponseInner::send_error(e))
        };
        let mut total_size = 0;
        let sent_size = Arc::new(AtomicU64::new(0));
        if let Some(body) = req.body_mut().take() {
            //优先从Content-Length获取
            let mut content_length = if let Some(len) = req.headers().get(CONTENT_LENGTH) {
                if let Ok(len) = len.to_str() {
                    len.parse::<u64>().ok()
                } else {
                    None
                }
            } else {
                None
            };
            if content_length.is_none() {
                content_length = body.size_hint().exact();
            }
            total_size = content_length.unwrap_or_default();
            //替换Body
            req.body_mut().replace(Body::wrap_stream(HttpBodyProgress::new(body, sent_size.clone())));
        }

        //定时器（每秒计算一次速率并回调通知对象）
        let mut tick_start = Instant::now();
        let mut tick_interval =
            time::interval_at(tick_start + Duration::from_secs(1), Duration::from_secs(1));
        let mut tick_size: u64 = 0; //基准
        let mut tick_invoke = Either::Left(future::pending());

        //完结回调事件流的标识
        #[derive(Debug, PartialEq, Eq)]
        enum DoneFlag {
            Pending,
            Invoke,
            Invoking,
            Done
        }
        let mut done_flag = DoneFlag::Pending;
        let mut resp = None;
        let mut req = Either::Left(raw_client.execute(req));

        loop {
            tokio::select! {
                res = &mut req => {
                    match res {
                        Ok(res) => {
                            assert_eq!(done_flag, DoneFlag::Pending);
                            resp = Some(res);
                            req = Either::Right(future::pending());
                            done_flag = DoneFlag::Invoke;
                            tokio::task::yield_now().await;
                            continue;
                        },
                        Err(e) => {
                            return Err(HttpResponseInner::send_error(e));
                        }
                    }
                },
                _ = tick_interval.tick() => {
                    let sent_size = sent_size.load(Ordering::SeqCst);
                    let speed = (sent_size - tick_size) as f32 / tick_start.elapsed().as_secs_f32();
                    tick_size = sent_size;
                    tick_start = Instant::now();
                    //UI线程阻塞时截流，丢弃中间的速率
                    if matches!(tick_invoke, Either::Left(_)) {
                        tick_invoke = Either::Right(
                            invoker.invoke(
                                        (id, total_size, sent_size, speed),
                                        |this, (id, total_size, sent_size, speed)| {
                                            this.on_send(
                                                id,
                                                total_size as pbulong,
                                                sent_size as pbulong,
                                                speed as pbulong
                                            )
                                        }
                                    )
                                    .then(|rv| {
                                        async {
                                            rv.await
                                        }
                                    })
                                    .boxed()
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
                                return Err(HttpResponseInner::cancelled());
                            }
                        },
                        Err(InvokeError::TargetIsDead) => return Err(HttpResponseInner::cancelled()),
                        Err(InvokeError::Panic) => panic!("Callback panic at OnSend")
                    }
                    #[allow(unused_assignments)]
                    if done_flag == DoneFlag::Invoking {
                        done_flag = DoneFlag::Done;
                        return Ok(resp.expect("Unexpected Response"));
                    }
                }
            }
        }
    }
}

struct HttpRequestInner {
    client: SharedObject,
    builder: Option<RequestBuilder>
}

/// 封装HttpBody捕获发送字节数
struct HttpBodyProgress {
    body: Body,
    sent_size: Arc<AtomicU64>
}

impl HttpBodyProgress {
    fn new(body: Body, sent_size: Arc<AtomicU64>) -> Self {
        HttpBodyProgress {
            body,
            sent_size
        }
    }
}

impl Stream for HttpBodyProgress {
    type Item = ReqwestResult<Bytes>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Option<Self::Item>> {
        match ready!(HttpBody::poll_frame(Pin::new(&mut self.body), cx)) {
            Some(res) => {
                match res {
                    Ok(res) => {
                        let data = res.into_data().expect("Unexpected streaming body");
                        self.sent_size.fetch_add(data.len() as u64, Ordering::SeqCst);
                        Poll::Ready(Some(Ok(data)))
                    },
                    Err(e) => Poll::Ready(Some(Err(e)))
                }
            },
            None => Poll::Ready(None)
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let hint = self.body.size_hint();
        (hint.lower() as usize, hint.upper().map(|v| v as usize))
    }
}
