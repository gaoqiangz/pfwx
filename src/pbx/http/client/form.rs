use std::mem::replace;

use super::*;

#[derive(Default)]
pub struct HttpForm {
    form: HashMap<String, String>
}

#[nonvisualobject(name = "nx_httpform")]
impl HttpForm {
    /// 创建`HashMap`
    ///
    /// # Notice
    ///
    /// 仅能调用一次
    pub fn build(&mut self) -> HashMap<String, String> { replace(&mut self.form, HashMap::default()) }

    #[method(name = "AddField")]
    fn field(&mut self, name: String, val: String) -> &mut Self {
        self.form.insert(name, val);
        self
    }
}
