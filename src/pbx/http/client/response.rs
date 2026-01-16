use std::{borrow::Cow, fmt::Display, time::Duration};

use bytes::{Bytes, BytesMut};
use futures_util::future::{self, Either, FutureExt};
use mime::Mime;
use reqwest::{
    header::{self, HeaderMap}, Response, StatusCode, Url, Version
};
use tokio::{
    fs::File, io::AsyncWriteExt, task::yield_now, time::{self, Instant}
};

use super::*;
use crate::{
    base::{conv, pfw}, reactor::HandlerInvoker
};

#[derive(Default)]
pub struct HttpResponse {
    inner: Option<HttpResponseInner>,
    elapsed: u128,
    async_id: Option<pbulong>,
    receive_file: Option<String>
}

#[nonvisualobject(name = "nx_httpresponse")]
impl HttpResponse {
    pub fn init(
        &mut self,
        kind: HttpResponseInner,
        elapsed: u128,
        async_id: Option<pbulong>,
        receive_file: Option<String>
    ) {
        self.inner = Some(kind);
        self.elapsed = elapsed;
        self.async_id = async_id;
        self.receive_file = receive_file;
    }

    fn status(&self) -> Option<StatusCode> {
        if let Some(inner) = self.inner.as_ref() {
            match inner {
                HttpResponseInner::ReceiveError {
                    status,
                    ..
                } => Some(*status),
                HttpResponseInner::Received {
                    status,
                    ..
                } => Some(*status),
                _ => None
            }
        } else {
            None
        }
    }

    fn headers(&self) -> Option<&HeaderMap> {
        if let Some(inner) = self.inner.as_ref() {
            match inner {
                HttpResponseInner::ReceiveError {
                    headers,
                    ..
                } => Some(headers),
                HttpResponseInner::Received {
                    headers,
                    ..
                } => Some(headers),
                _ => None
            }
        } else {
            None
        }
    }

    fn content_type(&self) -> Option<&Mime> {
        if let Some(inner) = self.inner.as_ref() {
            match inner {
                HttpResponseInner::ReceiveError {
                    content_type,
                    ..
                } => content_type.as_ref(),
                HttpResponseInner::Received {
                    content_type,
                    ..
                } => content_type.as_ref(),
                _ => None
            }
        } else {
            None
        }
    }

    fn data(&self) -> Option<&Bytes> {
        if let Some(inner) = self.inner.as_ref() {
            match inner {
                HttpResponseInner::Received {
                    data,
                    ..
                } => Some(&data),
                _ => None
            }
        } else {
            None
        }
    }

    fn error(&self) -> Option<&str> {
        if let Some(inner) = self.inner.as_ref() {
            match inner {
                HttpResponseInner::SendError {
                    err_info,
                    ..
                } => Some(&err_info),
                HttpResponseInner::ReceiveError {
                    err_info,
                    ..
                } => Some(&err_info),
                _ => None
            }
        } else {
            None
        }
    }

    #[method(name = "IsValid")]
    fn is_valid(&self) -> bool { self.inner.as_ref().map(HttpResponseInner::is_received).unwrap_or_default() }

    #[method(name = "IsHttpStatusOK")]
    fn is_http_status_ok(&self) -> bool {
        self.status().map(|status| status == StatusCode::OK).unwrap_or_default()
    }

    #[method(name = "IsCancelled")]
    fn is_cancelled(&self) -> bool {
        self.inner.as_ref().map(HttpResponseInner::is_cancelled).unwrap_or_default()
    }

    #[method(name = "IsText")]
    fn is_text(&self) -> bool {
        self.content_type()
            .map(|content_type| {
                content_type.type_() == "text" || content_type.subtype().as_str().ends_with("text")
            })
            .unwrap_or_default()
    }

    #[method(name = "IsJSON")]
    fn is_json(&self) -> bool {
        self.content_type()
            .map(|content_type| content_type.subtype().as_str().ends_with("json"))
            .unwrap_or_default()
    }

    #[method(name = "IsXML")]
    fn is_xml(&self) -> bool {
        self.content_type()
            .map(|content_type| {
                content_type.subtype().as_str().ends_with("xml") ||
                    content_type.suffix().map(|v| v == "xml").unwrap_or_default()
            })
            .unwrap_or_default()
    }

    #[method(name = "IsBinary")]
    fn is_binary(&self) -> bool {
        self.content_type()
            .map(|content_type| content_type.subtype().as_str().ends_with("stream"))
            .unwrap_or_default()
    }

    #[method(name = "IsAsync")]
    fn is_async(&self) -> bool { self.async_id.is_some() }

    #[method(name = "GetAsyncId")]
    fn id(&self) -> pbulong { self.async_id.unwrap_or_default() }

    #[method(name = "GetElapsed")]
    fn elapsed(&self) -> pbulong { self.elapsed as pbulong }

    #[method(name = "GetReceiveFile")]
    fn receive_file(&self) -> &str { self.receive_file.as_ref().map(|v| v.as_str()).unwrap_or_default() }

    #[method(name = "GetHeader")]
    fn header(&self, key: String) -> &str {
        self.headers().and_then(|headers| headers.get(key)).and_then(|v| v.to_str().ok()).unwrap_or_default()
    }

    #[method(name = "GetHeader")]
    fn header_by_index(&self, index: pbint) -> &str {
        self.headers()
            .and_then(|headers| headers.values().nth((index - 1) as usize))
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
    }

    #[method(name = "GetHeaderName")]
    fn header_name_by_index(&self, index: pbint) -> &str {
        self.headers()
            .and_then(|headers| headers.keys().nth((index - 1) as usize))
            .map(|v| v.as_str())
            .unwrap_or_default()
    }

    #[method(name = "GetHeaderCount")]
    fn header_count(&self) -> pbint {
        self.headers().map(|headers| headers.len()).unwrap_or_default() as pbint
    }

    #[method(name = "GetHeaders")]
    fn headers_serialize(&self) -> String {
        self.headers()
            .map(|headers| {
                headers.iter().map(|(k, v)| format!("{}={}\r\n", k, v.to_str().unwrap_or_default())).collect()
            })
            .unwrap_or_default()
    }

    #[method(name = "GetContentType")]
    fn content_type_serialize(&self) -> String {
        self.content_type().map(|content_type| content_type.to_string()).unwrap_or_default()
    }

    #[method(name = "GetCharset")]
    fn charset_serialize(&self) -> &str {
        self.content_type()
            .and_then(|content_type| content_type.get_param("charset"))
            .map(|v| v.as_str())
            .unwrap_or_default()
    }

    #[method(name = "GetUrl")]
    fn url(&self) -> String {
        if let Some(inner) = self.inner.as_ref() {
            match inner {
                HttpResponseInner::ReceiveError {
                    url,
                    ..
                } => url.to_string(),
                HttpResponseInner::Received {
                    url,
                    ..
                } => url.to_string(),
                _ => "".to_owned()
            }
        } else {
            "".to_owned()
        }
    }

    #[method(name = "GetHttpStatus")]
    fn http_status(&self) -> pbulong {
        self.status().map(|status| status.as_u16() as pbulong).unwrap_or_default()
    }

    #[method(name = "GetHttpVersion")]
    fn http_version(&self) -> String {
        if let Some(inner) = self.inner.as_ref() {
            match inner {
                HttpResponseInner::ReceiveError {
                    version,
                    ..
                } => format!("{:?}", version),
                HttpResponseInner::Received {
                    version,
                    ..
                } => format!("{:?}", version),
                _ => "".to_owned()
            }
        } else {
            "".to_owned()
        }
    }

    #[method(name = "GetErrorInfo")]
    fn error_info(&self) -> &str { self.error().unwrap_or_default() }

    #[method(name = "GetData")]
    fn data_binay(&self) -> &[u8] { self.data().map(Bytes::as_ref).unwrap_or_default() }

    #[method(name = "GetDataString", overload = 1)]
    fn data_string(&self, encoding: Option<pblong>) -> Cow<'_, str> {
        if let Some(data) = self.data() {
            match encoding {
                Some(encoding) => conv::decode(&data, encoding),
                None => {
                    let charset = self
                        .content_type()
                        .and_then(|content_type| content_type.get_param("charset"))
                        .map(|charset| charset.as_str())
                        .unwrap_or_default();
                    conv::decode_by_charset(&data, charset)
                }
            }
        } else {
            "".into()
        }
    }

    #[method(name = "GetDataJSON", overload = 1)]
    fn data_json(&self, encoding: Option<pblong>) -> Object {
        let data = if let Some(data) = self.data() {
            match encoding {
                Some(encoding) => conv::decode(&data, encoding),
                None => {
                    let charset = self
                        .content_type()
                        .and_then(|content_type| content_type.get_param("charset"))
                        .map(|charset| charset.as_str())
                        .unwrap_or_default();
                    conv::decode_by_charset(&data, charset)
                }
            }
        } else {
            "".into()
        };
        pfw::json_parse(self.get_session(), &data)
    }

    #[method(name = "GetDataXML", overload = 1)]
    fn data_xml(&self, encoding: Option<pblong>) -> Object {
        let data = if let Some(data) = self.data() {
            match encoding {
                Some(encoding) => conv::decode(&data, encoding),
                None => {
                    let charset = self
                        .content_type()
                        .and_then(|content_type| content_type.get_param("charset"))
                        .map(|charset| charset.as_str())
                        .unwrap_or_default();
                    conv::decode_by_charset(&data, charset)
                }
            }
        } else {
            "".into()
        };
        pfw::xml_parse(self.get_session(), &data)
    }
}

pub enum HttpResponseInner {
    SendError {
        err_info: String
    },
    ReceiveError {
        url: Url,
        version: Version,
        status: StatusCode,
        headers: HeaderMap,
        content_type: Option<Mime>,
        err_info: String
    },
    Received {
        url: Url,
        version: Version,
        status: StatusCode,
        headers: HeaderMap,
        content_type: Option<Mime>,
        data: Bytes
    },
    Cancelled
}

impl HttpResponseInner {
    pub fn is_send_error(&self) -> bool { matches!(self, HttpResponseInner::SendError { .. }) }
    pub fn is_receive_error(&self) -> bool { matches!(self, HttpResponseInner::ReceiveError { .. }) }
    pub fn is_received(&self) -> bool { matches!(self, HttpResponseInner::Received { .. }) }
    pub fn is_cancelled(&self) -> bool { matches!(self, HttpResponseInner::Cancelled) }
    pub fn is_succ(&self) -> bool { self.is_received() }

    pub fn cancelled() -> HttpResponseInner { HttpResponseInner::Cancelled }
    pub fn send_error(err_info: impl Display) -> HttpResponseInner {
        HttpResponseInner::SendError {
            err_info: err_info.to_string()
        }
    }
    fn receive_error(
        url: Url,
        version: Version,
        status: StatusCode,
        headers: HeaderMap,
        err_info: impl Display
    ) -> HttpResponseInner {
        let content_type = headers
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<Mime>().ok());
        HttpResponseInner::ReceiveError {
            url,
            version,
            status,
            headers,
            content_type,
            err_info: err_info.to_string()
        }
    }
    fn received(
        url: Url,
        version: Version,
        status: StatusCode,
        headers: HeaderMap,
        data: Bytes
    ) -> HttpResponseInner {
        let content_type = headers
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<Mime>().ok());
        HttpResponseInner::Received {
            url,
            version,
            status,
            headers,
            content_type,
            data
        }
    }

    pub async fn receive(mut resp: Response, recv_file_path: Option<String>) -> HttpResponseInner {
        let url = resp.url().clone();
        let version = resp.version();
        let status = resp.status();
        let headers = resp.headers().clone();
        if let Some(file_path) = recv_file_path {
            if let Err(e) = crate::base::fs::create_file_dir_all(&file_path) {
                HttpResponseInner::receive_error(url, version, status, headers, e)
            } else {
                match File::create(file_path).await {
                    Ok(mut file) => {
                        while let Some(chunk) = resp.chunk().await.transpose() {
                            match chunk {
                                Ok(chunk) => {
                                    if let Err(e) = file.write_all(&chunk).await {
                                        return HttpResponseInner::receive_error(
                                            url, version, status, headers, e
                                        );
                                    }
                                },
                                Err(e) => {
                                    return HttpResponseInner::receive_error(
                                        url, version, status, headers, e
                                    );
                                }
                            }
                        }
                        HttpResponseInner::received(url, version, status, headers, Default::default())
                    },
                    Err(e) => HttpResponseInner::receive_error(url, version, status, headers, e)
                }
            }
        } else {
            match resp.bytes().await {
                Ok(data) => HttpResponseInner::received(url, version, status, headers, data),
                Err(e) => HttpResponseInner::receive_error(url, version, status, headers, e)
            }
        }
    }

    pub async fn receive_with_progress(
        id: pbulong,
        invoker: HandlerInvoker<HttpClient>,
        mut resp: Response,
        recv_file_path: Option<String>
    ) -> HttpResponseInner {
        let url = resp.url().clone();
        let version = resp.version();
        let status = resp.status();
        let headers = resp.headers().clone();
        let mut file = if let Some(file_path) = recv_file_path {
            if let Err(e) = crate::base::fs::create_file_dir_all(&file_path) {
                return HttpResponseInner::receive_error(url, version, status, headers, e);
            } else {
                match File::create(file_path).await {
                    Ok(file) => Some(file),
                    Err(e) => return HttpResponseInner::receive_error(url, version, status, headers, e)
                }
            }
        } else {
            None
        };

        let total_size = resp.content_length().unwrap_or_default();
        let mut recv_size: u64 = 0;
        let mut recv_data = if file.is_some() {
            BytesMut::new()
        } else {
            BytesMut::with_capacity(total_size.max(1024 * 1024) as usize)
        };

        // 定时器（每秒计算一次速率并回调通知对象）
        let mut tick_start = Instant::now();
        let mut tick_interval =
            time::interval_at(tick_start + Duration::from_secs(1), Duration::from_secs(1));
        let mut tick_size: u64 = 0; // 基准
        let mut tick_invoke = Either::Left(future::pending());

        // 完结回调事件流的标识
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
                            recv_size += chunk.len() as u64;
                            if let Some(file) = file.as_mut() {
                                if let Err(e) = file.write_all(&chunk).await {
                                    return HttpResponseInner::receive_error(url, version, status, headers,  e);
                                }
                            } else {
                                recv_data.extend_from_slice(&chunk);
                            }
                        },
                        Ok(None) => {
                            if done_flag == DoneFlag::Pending {
                                done_flag = DoneFlag::Invoke;
                            }
                            if done_flag == DoneFlag::Invoke || done_flag == DoneFlag::Invoking {
                                yield_now().await;
                                continue;
                            }
                            return HttpResponseInner::received(url, version, status, headers,  recv_data.freeze());
                        },
                        Err(e) => {
                            return HttpResponseInner::receive_error(url, version, status, headers, e);
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
                            invoker.invoke(
                                        (id, total_size, recv_size, speed),
                                        |this, (id, total_size, recv_size, speed)| {
                                            this.on_recv(
                                                id,
                                                total_size as pbulong,
                                                recv_size as pbulong,
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
                                return HttpResponseInner::cancelled();
                            }
                        },
                        Err(InvokeError::TargetIsDead) => return HttpResponseInner::cancelled(),
                        Err(InvokeError::Panic) => panic!("Callback panic at OnRecv")
                    }
                    if done_flag == DoneFlag::Invoking {
                        done_flag = DoneFlag::Done;
                    }
                }
            }
        }
    }
}
