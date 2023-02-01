use super::*;
use reqwest::cookie::{CookieStore, Jar};

#[derive(Default)]
pub struct HttpCookie {
    jar: Arc<Jar>
}

#[nonvisualobject(name = "nx_httpcookie")]
impl HttpCookie {
    /// 获取`Cookie-Jar`
    pub fn get(&self) -> Arc<Jar> { self.jar.clone() }

    #[method(name = "SetCookie")]
    fn set_cookie(&mut self, url: String, cookie: String) -> &mut Self {
        if let Ok(url) = &url.parse() {
            self.jar.add_cookie_str(&cookie, url);
        }
        self
    }

    #[method(name = "GetCookie")]
    fn get_cookie(&self, url: String) -> String {
        if let Ok(url) = &url.parse() {
            if let Some(cookie) = self.jar.cookies(url) {
                cookie.to_str().map(String::from).unwrap_or_default()
            } else {
                Default::default()
            }
        } else {
            Default::default()
        }
    }
}
