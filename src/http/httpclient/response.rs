use super::*;
use bytes::Bytes;
use reqwest::{header::HeaderMap, StatusCode};

#[derive(Default)]
pub struct HttpResponse {
    inner: Option<HttpResponseKind>
}

#[nonvisualobject(name = "nx_httpresponse")]
impl HttpResponse {
    pub fn init(&mut self, kind: HttpResponseKind) { self.inner = Some(kind); }
}

pub enum HttpResponseKind {
    SendError {
        err_info: String
    },
    ReceiveError {
        status: StatusCode,
        headers: HeaderMap,
        err_info: String
    },
    Received {
        status: StatusCode,
        headers: HeaderMap,
        data: Bytes
    },
    Cancelled
}

impl HttpResponseKind {
    pub fn send_error(err_info: String) -> HttpResponseKind {
        HttpResponseKind::SendError {
            err_info
        }
    }

    pub fn receive_error(status: StatusCode, headers: HeaderMap, err_info: String) -> HttpResponseKind {
        HttpResponseKind::ReceiveError {
            status,
            headers,
            err_info
        }
    }

    pub fn received(status: StatusCode, headers: HeaderMap, data: Bytes) -> HttpResponseKind {
        HttpResponseKind::Received {
            status,
            headers,
            data
        }
    }

    pub fn cancelled() -> HttpResponseKind { HttpResponseKind::Cancelled }
}
