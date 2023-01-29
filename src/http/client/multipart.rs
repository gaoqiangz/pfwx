use super::*;
use reqwest::multipart::{Form, Part};
use std::fs::File as StdFile;
use tokio::fs::File;

pub struct HttpMultipart {
    builder: Option<Form>
}

impl Default for HttpMultipart {
    fn default() -> Self {
        HttpMultipart {
            builder: Some(Form::default())
        }
    }
}

#[nonvisualobject(name = "nx_httpmultipart")]
impl HttpMultipart {
    /// 创建`Form`
    ///
    /// # Notice
    ///
    /// 仅能调用一次
    pub fn build(&mut self) -> Form { self.builder.replace(Form::default()).unwrap() }

    #[method(name = "AddField", overload = 1)]
    fn text(&mut self, name: String, val: String, mime: Option<String>) -> &mut Self {
        let mut part = Part::text(val);
        if let Some(mime) = mime {
            part = part.mime_str(mime.as_str()).expect("invalid mime");
        }
        let builder = self.builder.take().unwrap();
        self.builder.replace(builder.part(name, part));
        self
    }

    #[method(name = "AddField", overload = 1)]
    fn binary(&mut self, name: String, val: &[u8], mime: Option<String>) -> &mut Self {
        let len = val.len();
        let mut part = Part::stream_with_length(val.to_owned(), len as u64);
        if let Some(mime) = mime {
            part = part.mime_str(mime.as_str()).expect("invalid mime");
        }
        let builder = self.builder.take().unwrap();
        self.builder.replace(builder.part(name, part));
        self
    }

    #[method(name = "AddFile", overload = 2)]
    fn file(
        &mut self,
        name: String,
        file_path: String,
        file_name: Option<String>,
        mime: Option<String>
    ) -> &mut Self {
        if let Ok(file) = StdFile::open(file_path) {
            let len = file.metadata().unwrap().len();
            let mut part = Part::stream_with_length(File::from_std(file), len);
            if let Some(file_name) = file_name {
                part = part.file_name(file_name);
            }
            if let Some(mime) = mime {
                part = part.mime_str(mime.as_str()).expect("invalid mime");
            }
            let builder = self.builder.take().unwrap();
            self.builder.replace(builder.part(name, part));
        }
        self
    }

    #[method(name = "GetBoundary")]
    fn boundary(&mut self) -> &str { self.builder.as_ref().unwrap().boundary() }
}
