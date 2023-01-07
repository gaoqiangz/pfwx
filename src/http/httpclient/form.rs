use super::*;
use std::collections::HashMap;

#[derive(Default)]
pub struct HttpForm {
    builder: Option<HashMap<String, String>>
}

#[nonvisualobject(name = "nx_httpform")]
impl HttpForm {
    /// 创建`HashMap`
    ///
    /// # Notice
    ///
    /// 仅能调用一次
    pub fn build(&mut self) -> HashMap<String, String> { self.builder.replace(HashMap::default()).unwrap() }

    #[method(name = "AddField")]
    fn field(&mut self, name: String, val: String) -> &mut Self {
        let builder = self.builder.as_mut().unwrap();
        builder.insert(name, val);
        self
    }
}
