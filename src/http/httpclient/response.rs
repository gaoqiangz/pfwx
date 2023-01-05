use super::*;
use bytes::Bytes;
use mime::Mime;
use reqwest::{
    header::{self, HeaderMap}, StatusCode
};
use std::{borrow::Cow, fmt::Display};

#[derive(Default)]
pub struct HttpResponse {
    inner: Option<HttpResponseKind>
}

#[nonvisualobject(name = "nx_httpresponse")]
impl HttpResponse {
    pub fn init(&mut self, kind: HttpResponseKind) { self.inner = Some(kind); }

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
        self.content_type().map(|content_type| content_type.type_() == "text").unwrap_or_default()
    }

    #[method(name = "IsJSON")]
    fn is_json(&self) -> bool {
        self.content_type().map(|content_type| content_type.subtype() == "json").unwrap_or_default()
    }

    #[method(name = "IsXML")]
    fn is_xml(&self) -> bool {
        self.content_type()
            .map(|content_type| {
                content_type.subtype() == "xml" ||
                    content_type.suffix().map(|v| v == "xml").unwrap_or_default()
            })
            .unwrap_or_default()
    }

    #[method(name = "GetHeader")]
    fn header(&self, key: String) -> String {
        self.headers()
            .and_then(|headers| headers.get(key))
            .and_then(|v| v.to_str().ok())
            .map(|v| v.to_owned())
            .unwrap_or_default()
    }

    #[method(name = "GetHeader")]
    fn header_by_index(&self, index: pbint) -> String {
        self.headers()
            .and_then(|headers| headers.values().nth((index - 1) as usize))
            .and_then(|v| v.to_str().ok())
            .map(|v| v.to_owned())
            .unwrap_or_default()
    }

    #[method(name = "GetHeaderName")]
    fn header_name_by_index(&self, index: pbint) -> String {
        self.headers()
            .and_then(|headers| headers.keys().nth((index - 1) as usize))
            .map(|v| v.to_string())
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
    fn charset_serialize(&self) -> String {
        self.content_type()
            .and_then(|content_type| content_type.get_param("charset"))
            .map(|v| v.to_string())
            .unwrap_or_default()
    }

    #[method(name = "GetHttpStatus")]
    fn http_status(&self) -> pbulong {
        self.status().map(|status| status.as_u16() as pbulong).unwrap_or_default()
    }

    #[method(name = "GetErrorInfo")]
    fn error_info(&self) -> String { self.error().map(String::from).unwrap_or_default() }

    #[method(name = "GetData")]
    fn data_blob(&self) -> &[u8] { self.data().map(Bytes::as_ref).unwrap_or_default() }

    #[method(name = "GetDataString", overload = 1)]
    fn data_string(&self, encoding: Option<pblong>) -> Cow<'_, str> {
        if let Some(data) = self.data() {
            let codec = match encoding {
                Some(code) => {
                    encoding::label::encoding_from_windows_code_page(encoding_conv::conv_codepage(code))
                },
                None => {
                    self.content_type()
                        .and_then(|content_type| content_type.get_param("charset"))
                        .and_then(|charset| encoding::label::encoding_from_whatwg_label(charset.as_str()))
                },
            };
            let codec = codec.unwrap_or(encoding::all::UTF_8);
            if codec.name() == "utf-8" {
                String::from_utf8_lossy(&data)
            } else {
                codec.decode(&data, encoding::DecoderTrap::Replace).map(Cow::from).unwrap_or_default()
            }
        } else {
            "".into()
        }
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

mod encoding_conv {
    use pbni::primitive::pblong;

    const ENCODING_UNKNOWN: pblong = 0;
    const ENCODING_UTF8: pblong = 1;
    const ENCODING_UTF16: pblong = 2;
    const ENCODING_UTF16LE: pblong = 2;
    const ENCODING_UTF16BE: pblong = 3;
    const ENCODING_ANSI: pblong = 4;
    const ENCODING_GB2312: pblong = 5;
    const ENCODING_GBK: pblong = 5;
    const ENCODING_GB18030: pblong = 6;
    const ENCODING_BIG5: pblong = 7;
    const ENCODING_ISO88591: pblong = 8;
    const ENCODING_LATIN1: pblong = 8;
    const ENCODING_ISO88592: pblong = 9;
    const ENCODING_LATIN2: pblong = 9;
    const ENCODING_ISO88593: pblong = 10;
    const ENCODING_LATIN3: pblong = 10;
    const ENCODING_ISO2022JP: pblong = 11;
    const ENCODING_ISO2022KR: pblong = 12;

    pub fn conv_codepage(encoding: pblong) -> usize {
        match encoding {
            ENCODING_ANSI => 0,
            ENCODING_UTF8 => 65001,
            ENCODING_UTF16LE => 1200,
            ENCODING_UTF16BE => 1201,
            ENCODING_GB2312 => 936,
            ENCODING_GB18030 => 54936,
            ENCODING_BIG5 => 950,
            ENCODING_ISO88591 => 28591,
            ENCODING_ISO88592 => 28592,
            ENCODING_ISO88593 => 28593,
            ENCODING_ISO2022JP => 50220,
            ENCODING_ISO2022KR => 50225,
            _ => 0
        }
    }
}
