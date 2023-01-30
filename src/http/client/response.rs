use super::*;
use crate::base::{conv, pfw};
use bytes::Bytes;
use mime::Mime;
use reqwest::{
    header::{self, HeaderMap}, StatusCode
};
use std::{borrow::Cow, fmt::Display};

#[derive(Default)]
pub struct HttpResponse {
    inner: Option<HttpResponseKind>,
    elapsed: u128,
    async_id: Option<pbulong>,
    receive_file: Option<String>
}

#[nonvisualobject(name = "nx_httpresponse")]
impl HttpResponse {
    pub fn init(
        &mut self,
        kind: HttpResponseKind,
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
                HttpResponseKind::ReceiveError {
                    status,
                    ..
                } => Some(*status),
                HttpResponseKind::Received {
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
                HttpResponseKind::ReceiveError {
                    headers,
                    ..
                } => Some(headers),
                HttpResponseKind::Received {
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
                HttpResponseKind::ReceiveError {
                    content_type,
                    ..
                } => content_type.as_ref(),
                HttpResponseKind::Received {
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
                HttpResponseKind::Received {
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
                HttpResponseKind::SendError {
                    err_info,
                    ..
                } => Some(&err_info),
                HttpResponseKind::ReceiveError {
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
    fn is_valid(&self) -> bool { self.inner.as_ref().map(HttpResponseKind::is_received).unwrap_or_default() }

    #[method(name = "IsHttpStatusOK")]
    fn is_http_status_ok(&self) -> bool {
        self.status().map(|status| status == StatusCode::OK).unwrap_or_default()
    }

    #[method(name = "IsCancelled")]
    fn is_cancelled(&self) -> bool {
        self.inner.as_ref().map(HttpResponseKind::is_cancelled).unwrap_or_default()
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

    #[method(name = "GetHttpStatus")]
    fn http_status(&self) -> pbulong {
        self.status().map(|status| status.as_u16() as pbulong).unwrap_or_default()
    }

    #[method(name = "GetErrorInfo")]
    fn error_info(&self) -> &str { self.error().unwrap_or_default() }

    #[method(name = "GetData")]
    fn data_blob(&self) -> &[u8] { self.data().map(Bytes::as_ref).unwrap_or_default() }

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

pub enum HttpResponseKind {
    SendError {
        err_info: String
    },
    ReceiveError {
        status: StatusCode,
        headers: HeaderMap,
        content_type: Option<Mime>,
        err_info: String
    },
    Received {
        status: StatusCode,
        headers: HeaderMap,
        content_type: Option<Mime>,
        data: Bytes
    },
    Cancelled
}

impl HttpResponseKind {
    pub fn is_send_error(&self) -> bool { matches!(self, HttpResponseKind::SendError { .. }) }
    pub fn is_receive_error(&self) -> bool { matches!(self, HttpResponseKind::ReceiveError { .. }) }
    pub fn is_received(&self) -> bool { matches!(self, HttpResponseKind::Received { .. }) }
    pub fn is_cancelled(&self) -> bool { matches!(self, HttpResponseKind::Cancelled) }
    pub fn is_succ(&self) -> bool { self.is_received() }

    pub fn send_error(err_info: impl Display) -> HttpResponseKind {
        HttpResponseKind::SendError {
            err_info: err_info.to_string()
        }
    }
    pub fn receive_error(status: StatusCode, headers: HeaderMap, err_info: impl Display) -> HttpResponseKind {
        let content_type = headers
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<Mime>().ok());
        HttpResponseKind::ReceiveError {
            status,
            headers,
            content_type,
            err_info: err_info.to_string()
        }
    }
    pub fn received(status: StatusCode, headers: HeaderMap, data: Bytes) -> HttpResponseKind {
        let content_type = headers
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<Mime>().ok());
        HttpResponseKind::Received {
            status,
            headers,
            content_type,
            data
        }
    }

    pub fn cancelled() -> HttpResponseKind { HttpResponseKind::Cancelled }
}
