use super::*;
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue}, Certificate, ClientBuilder, Identity, Proxy
};
use std::time::Duration;

pub struct HttpClientRuntimeConfig {
    /// 异步请求-保证按调用顺序执行
    pub guarantee_order: bool
}

impl Default for HttpClientRuntimeConfig {
    fn default() -> Self {
        HttpClientRuntimeConfig {
            guarantee_order: true
        }
    }
}

pub struct HttpClientConfig {
    ctx: ContextObject,
    builder: Option<ClientBuilder>,
    rt_cfg: Option<HttpClientRuntimeConfig>
}

#[nonvisualobject(name = "nx_httpconfig")]
impl HttpClientConfig {
    #[constructor]
    fn new(_session: Session, ctx: ContextObject) -> Self {
        HttpClientConfig {
            ctx,
            builder: Some(Self::default_builder()),
            rt_cfg: Some(HttpClientRuntimeConfig::default())
        }
    }

    fn default_builder() -> ClientBuilder { ClientBuilder::default().use_native_tls() }

    /// 创建`reqwest::Client`
    ///
    /// # Notice
    ///
    /// 仅能调用一次
    pub fn build(&mut self) -> reqwest::Result<(Client, HttpClientRuntimeConfig)> {
        let client = self.builder.replace(Self::default_builder()).unwrap().build()?;
        let rt_cfg = self.rt_cfg.replace(HttpClientRuntimeConfig::default()).unwrap();
        Ok((client, rt_cfg))
    }

    #[method(name = "SetAgent")]
    fn agent(&mut self, val: String) -> &ContextObject {
        let builder = self.builder.take().unwrap();
        self.builder.replace(builder.user_agent(val));
        &self.ctx
    }

    #[method(name = "SetDefaultHeader")]
    fn default_header(&mut self, key: String, val: String) -> &ContextObject {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_str(&key).expect("invalid header key"),
            HeaderValue::from_str(&val).expect("invalid header value")
        );
        let builder = self.builder.take().unwrap();
        self.builder.replace(builder.default_headers(headers));
        &self.ctx
    }

    #[method(name = "SetCookieStore")]
    fn cookie_store(&mut self, enabled: bool) -> &ContextObject {
        let builder = self.builder.take().unwrap();
        self.builder.replace(builder.cookie_store(enabled));
        &self.ctx
    }

    #[method(name = "SetProxy")]
    fn proxy(&mut self, url: String) -> &ContextObject {
        let builder = self.builder.take().unwrap();
        self.builder.replace(builder.proxy(Proxy::all(url).expect("invalid proxy url")));
        &self.ctx
    }

    #[method(name = "SetProxy")]
    fn proxy_with_cred(&mut self, url: String, user: String, psw: String) -> &ContextObject {
        let builder = self.builder.take().unwrap();
        self.builder
            .replace(builder.proxy(Proxy::all(url).expect("invalid proxy url").basic_auth(&user, &psw)));
        &self.ctx
    }

    #[method(name = "AddRootCertificate")]
    fn add_root_certificate(&mut self, pem: String) -> &ContextObject {
        let builder = self.builder.take().unwrap();
        self.builder.replace(
            builder.add_root_certificate(
                Certificate::from_pem(pem.as_bytes()).expect("invalid root certificate")
            )
        );
        &self.ctx
    }

    #[method(name = "SetSysRootCertificate")]
    fn sys_root_certificate(&mut self, enabled: bool) -> &ContextObject {
        let builder = self.builder.take().unwrap();
        self.builder.replace(builder.tls_built_in_root_certs(enabled));
        &self.ctx
    }

    #[method(name = "SetCertificate")]
    fn certificate_pkcs8(&mut self, pem: String, key: String) -> &ContextObject {
        let builder = self.builder.take().unwrap();
        self.builder.replace(builder.identity(
            Identity::from_pkcs8_pem(pem.as_bytes(), key.as_bytes()).expect("invalid certificate (PKCS8)")
        ));
        &self.ctx
    }

    #[method(name = "SetCertificatePKCS12")]
    fn certificate_pkcs12(&mut self, der: &[u8], psw: String) -> &ContextObject {
        let builder = self.builder.take().unwrap();
        self.builder.replace(
            builder.identity(
                Identity::from_pkcs12_der(der, psw.as_str()).expect("invalid certificate (PKCS12)")
            )
        );
        &self.ctx
    }

    #[method(name = "AcceptInvalidCert")]
    fn accept_invalid_certs(&mut self, enabled: bool) -> &ContextObject {
        let builder = self.builder.take().unwrap();
        self.builder.replace(builder.danger_accept_invalid_certs(enabled));
        &self.ctx
    }

    #[method(name = "AcceptInvalidHost")]
    fn accept_invalid_hostnames(&mut self, enabled: bool) -> &ContextObject {
        let builder = self.builder.take().unwrap();
        self.builder.replace(builder.danger_accept_invalid_hostnames(enabled));
        &self.ctx
    }

    #[method(name = "SetTimeout")]
    fn timeout(&mut self, secs: pbdouble) -> &ContextObject {
        let builder = self.builder.take().unwrap();
        self.builder.replace(builder.timeout(Duration::from_secs_f64(secs)));
        &self.ctx
    }

    #[method(name = "SetConnectTimeout")]
    fn connect_timeout(&mut self, secs: pbdouble) -> &ContextObject {
        let builder = self.builder.take().unwrap();
        self.builder.replace(builder.connect_timeout(Duration::from_secs_f64(secs)));
        &self.ctx
    }

    #[method(name = "SetHttpsOnly")]
    fn https_only(&mut self, enabled: bool) -> &ContextObject {
        let builder = self.builder.take().unwrap();
        self.builder.replace(builder.https_only(enabled));
        &self.ctx
    }

    #[method(name = "SetGuaranteeOrder")]
    fn guarantee_order(&mut self, enabled: bool) -> &ContextObject {
        let mut rt_cfg = self.rt_cfg.take().unwrap();
        rt_cfg.guarantee_order = enabled;
        self.rt_cfg.replace(rt_cfg);
        &self.ctx
    }
}
