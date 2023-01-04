use super::*;
use reqwest::{header, Certificate, ClientBuilder, Identity, Proxy};
use std::time::Duration;

#[derive(Default)]
pub struct HttpClientRuntimeConfig {
    /// 异步请求保证按调用顺序执行
    pub guarantee_order: bool
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

    #[method]
    fn agent(&mut self, val: String) -> &ContextObject {
        let builder = self.builder.take().unwrap();
        self.builder.replace(builder.user_agent(val));
        &self.ctx
    }

    #[method]
    fn default_header(&mut self, key: String, val: String) -> &ContextObject {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::HeaderName::from_str(&key).expect("invalid header key"),
            header::HeaderValue::from_str(&val).expect("invalid header value")
        );
        let builder = self.builder.take().unwrap();
        self.builder.replace(builder.default_headers(headers));
        &self.ctx
    }

    #[method]
    fn cookie_store(&mut self, enabled: bool) -> &ContextObject {
        let builder = self.builder.take().unwrap();
        self.builder.replace(builder.cookie_store(enabled));
        &self.ctx
    }

    #[method]
    fn proxy(&mut self, url: String) -> &ContextObject {
        let builder = self.builder.take().unwrap();
        self.builder.replace(builder.proxy(Proxy::all(url).expect("invalid proxy url")));
        &self.ctx
    }

    #[method]
    fn proxy_with_cred(&mut self, url: String, user: String, psw: String) -> &ContextObject {
        let builder = self.builder.take().unwrap();
        self.builder
            .replace(builder.proxy(Proxy::all(url).expect("invalid proxy url").basic_auth(&user, &psw)));
        &self.ctx
    }

    #[method]
    fn add_root_certificate(&mut self, pem: String) -> &ContextObject {
        let builder = self.builder.take().unwrap();
        self.builder.replace(
            builder.add_root_certificate(
                Certificate::from_pem(pem.as_bytes()).expect("invalid root certificate")
            )
        );
        &self.ctx
    }

    #[method]
    fn sys_root_certificate(&mut self, enabled: bool) -> &ContextObject {
        let builder = self.builder.take().unwrap();
        self.builder.replace(builder.tls_built_in_root_certs(enabled));
        &self.ctx
    }

    #[method]
    fn certificate_pkcs8(&mut self, pem: String, key: String) -> &ContextObject {
        let builder = self.builder.take().unwrap();
        self.builder.replace(builder.identity(
            Identity::from_pkcs8_pem(pem.as_bytes(), key.as_bytes()).expect("invalid certificate (PKCS8)")
        ));
        &self.ctx
    }

    #[method]
    fn certificate_pkcs12(&mut self, der: &[u8], key: String) -> &ContextObject {
        let builder = self.builder.take().unwrap();
        self.builder.replace(
            builder.identity(
                Identity::from_pkcs12_der(der, key.as_str()).expect("invalid certificate (PKCS12)")
            )
        );
        &self.ctx
    }

    #[method]
    fn accept_invalid_certs(&mut self, enabled: bool) -> &ContextObject {
        let builder = self.builder.take().unwrap();
        self.builder.replace(builder.danger_accept_invalid_certs(enabled));
        &self.ctx
    }

    #[method]
    fn accept_invalid_hostnames(&mut self, enabled: bool) -> &ContextObject {
        let builder = self.builder.take().unwrap();
        self.builder.replace(builder.danger_accept_invalid_hostnames(enabled));
        &self.ctx
    }

    #[method]
    fn timeout(&mut self, secs: pbdouble) -> &ContextObject {
        let builder = self.builder.take().unwrap();
        self.builder.replace(builder.timeout(Duration::from_secs_f64(secs)));
        &self.ctx
    }

    #[method]
    fn connect_timeout(&mut self, secs: pbdouble) -> &ContextObject {
        let builder = self.builder.take().unwrap();
        self.builder.replace(builder.connect_timeout(Duration::from_secs_f64(secs)));
        &self.ctx
    }

    #[method]
    fn https_only(&mut self, enabled: bool) -> &ContextObject {
        let builder = self.builder.take().unwrap();
        self.builder.replace(builder.https_only(enabled));
        &self.ctx
    }

    #[method]
    fn guarantee_order(&mut self, enabled: bool) -> &ContextObject {
        let mut rt_cfg = self.rt_cfg.take().unwrap();
        rt_cfg.guarantee_order = enabled;
        self.rt_cfg.replace(rt_cfg);
        &self.ctx
    }
}
