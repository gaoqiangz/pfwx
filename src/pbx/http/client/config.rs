use std::time::Duration;

use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue}, Certificate, ClientBuilder, Identity, Proxy
};

use super::{cookie::HttpCookie, *};

pub struct HttpClientConfigEx {
    /// 异步请求-最大并发数
    pub max_concurrency: usize
}

impl Default for HttpClientConfigEx {
    fn default() -> Self {
        HttpClientConfigEx {
            max_concurrency: default::MAX_CONCURRENCY
        }
    }
}

pub struct HttpClientConfig {
    builder: Option<ClientBuilder>,
    cfg: Option<HttpClientConfigEx>
}

impl Default for HttpClientConfig {
    fn default() -> Self {
        HttpClientConfig {
            builder: Some(HttpClientConfig::default_builder()),
            cfg: Some(HttpClientConfigEx::default())
        }
    }
}

#[nonvisualobject(name = "nx_httpconfig")]
impl HttpClientConfig {
    fn default_builder() -> ClientBuilder { ClientBuilder::default().use_native_tls() }

    /// 创建`reqwest::Client`
    ///
    /// # Notice
    ///
    /// 仅能调用一次
    pub fn build(&mut self) -> reqwest::Result<(Client, HttpClientConfigEx)> {
        let builder = self.builder.replace(Self::default_builder()).unwrap();
        let rt_cfg = self.cfg.replace(HttpClientConfigEx::default()).unwrap();
        let client = builder.build()?;
        Ok((client, rt_cfg))
    }

    #[method(name = "SetAgent")]
    fn agent(&mut self, val: String) -> &mut Self {
        let builder = self.builder.take().unwrap();
        self.builder.replace(builder.user_agent(val));
        self
    }

    #[method(name = "SetDefaultHeader")]
    fn default_header(&mut self, key: String, val: String) -> &mut Self {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_str(&key).expect("invalid header key"),
            HeaderValue::from_str(&val).expect("invalid header value")
        );
        let builder = self.builder.take().unwrap();
        self.builder.replace(builder.default_headers(headers));
        self
    }

    #[method(name = "SetCookieStore")]
    fn cookie_store(&mut self, enabled: bool) -> &mut Self {
        let builder = self.builder.take().unwrap();
        self.builder.replace(builder.cookie_store(enabled));
        self
    }

    #[method(name = "SetCookieStore")]
    fn cookie_provider(&mut self, store: &HttpCookie) -> &mut Self {
        let builder = self.builder.take().unwrap();
        self.builder.replace(builder.cookie_provider(store.get()));
        self
    }

    #[method(name = "SetProxy")]
    fn proxy(&mut self, url: String) -> &mut Self {
        let builder = self.builder.take().unwrap();
        self.builder.replace(builder.proxy(Proxy::all(url).expect("invalid proxy url")));
        self
    }

    #[method(name = "SetProxy")]
    fn proxy_with_cred(&mut self, url: String, user: String, psw: String) -> &mut Self {
        let builder = self.builder.take().unwrap();
        self.builder
            .replace(builder.proxy(Proxy::all(url).expect("invalid proxy url").basic_auth(&user, &psw)));
        self
    }

    #[method(name = "AddRootCertificate")]
    fn add_root_certificate(&mut self, pem: String) -> &mut Self {
        let builder = self.builder.take().unwrap();
        self.builder.replace(
            builder.add_root_certificate(
                Certificate::from_pem(pem.as_bytes()).expect("invalid root certificate")
            )
        );
        self
    }

    #[method(name = "SetSysRootCertificate")]
    fn sys_root_certificate(&mut self, enabled: bool) -> &mut Self {
        let builder = self.builder.take().unwrap();
        self.builder.replace(builder.tls_built_in_root_certs(enabled));
        self
    }

    #[method(name = "SetCertificate")]
    fn certificate_pkcs8(&mut self, pem: String, key: String) -> &mut Self {
        let builder = self.builder.take().unwrap();
        self.builder.replace(builder.identity(
            Identity::from_pkcs8_pem(pem.as_bytes(), key.as_bytes()).expect("invalid certificate (PKCS8)")
        ));
        self
    }

    #[method(name = "SetCertificatePKCS12")]
    fn certificate_pkcs12(&mut self, der: &[u8], psw: String) -> &mut Self {
        let builder = self.builder.take().unwrap();
        self.builder.replace(
            builder.identity(
                Identity::from_pkcs12_der(der, psw.as_str()).expect("invalid certificate (PKCS12)")
            )
        );
        self
    }

    #[method(name = "AcceptInvalidCert")]
    fn accept_invalid_certs(&mut self, enabled: bool) -> &mut Self {
        let builder = self.builder.take().unwrap();
        self.builder.replace(builder.danger_accept_invalid_certs(enabled));
        self
    }

    #[method(name = "AcceptInvalidHost")]
    fn accept_invalid_hostnames(&mut self, enabled: bool) -> &mut Self {
        let builder = self.builder.take().unwrap();
        self.builder.replace(builder.danger_accept_invalid_hostnames(enabled));
        self
    }

    #[method(name = "SetTimeout")]
    fn timeout(&mut self, secs: pbdouble) -> &mut Self {
        let builder = self.builder.take().unwrap();
        self.builder.replace(builder.timeout(Duration::from_secs_f64(secs)));
        self
    }

    #[method(name = "SetConnectTimeout")]
    fn connect_timeout(&mut self, secs: pbdouble) -> &mut Self {
        let builder = self.builder.take().unwrap();
        self.builder.replace(builder.connect_timeout(Duration::from_secs_f64(secs)));
        self
    }

    #[method(name = "SetHttpsOnly")]
    fn https_only(&mut self, enabled: bool) -> &mut Self {
        let builder = self.builder.take().unwrap();
        self.builder.replace(builder.https_only(enabled));
        self
    }

    #[method(name = "SetConcurrency")]
    fn concurrency(&mut self, max_concurrency: u32) -> &mut Self {
        let mut rt_cfg = self.cfg.take().unwrap();
        rt_cfg.max_concurrency = max_concurrency.max(1) as usize;
        self.cfg.replace(rt_cfg);
        self
    }
}

/// 默认配置
pub mod default {
    /// 异步请求-最大并发数
    pub const MAX_CONCURRENCY: usize = 16;
}
