use super::*;
use reqwest::multipart::{Form, Part};

#[derive(Default)]
pub struct HttpForm {
    builder: Form
}

#[nonvisualobject(name = "nx_httpform")]
impl HttpForm {}
