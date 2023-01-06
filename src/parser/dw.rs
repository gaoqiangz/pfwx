use crate::prelude::*;
use dwparser::DWSyntax;
use pbni::pbx::*;
use std::mem::transmute;

#[derive(Default)]
struct DWParser {
    inner: Option<DWParserInner>
}

#[nonvisualobject(name = "nx_dwparser")]
impl DWParser {
    /// 从DW语法解析
    ///
    /// 支持`.srd`文件格式
    #[method(name = "Parse")]
    fn parse(&mut self, syn: String) -> RetCode {
        let syn_ref: &'static str = unsafe {
            //SAFETY
            transmute(syn.as_str())
        };
        let ast = DWSyntax::parse(syn_ref)?;
        self.inner = Some(DWParserInner {
            syn,
            ast
        });
        RetCode::OK
    }

    /// 获取指定语法项的参数值
    ///
    /// 兼容`DataWindow::Describe`参数和返回值
    #[method(name = "Describe")]
    fn describe(&self, selector: String) -> String {
        if let Some(inner) = &self.inner {
            inner.ast.describe(&selector)
        } else {
            "!".to_owned()
        }
    }

    /// 修改语法项的参数值
    ///
    /// 兼容`DataWindow::Modify`参数和返回值
    #[method(name = "Modify")]
    fn modify(&mut self, modifier: String) -> String {
        if let Some(inner) = &mut self.inner {
            inner.ast.modify(&modifier)
        } else {
            "!".to_owned()
        }
    }

    /// 反序列化`JSON-AST`字符串
    #[method(name = "FromJson")]
    fn from_json_ast(&mut self, syn: String) -> RetCode {
        let syn_ref: &'static str = unsafe {
            //SAFETY
            transmute(syn.as_str())
        };
        let ast = serde_json::from_str::<DWSyntax>(syn_ref)?;
        self.inner = Some(DWParserInner {
            syn,
            ast
        });
        RetCode::OK
    }

    /// 序列化为`JSON-AST`字符串
    #[method(name = "ToJson")]
    fn to_json_ast(&self) -> String {
        if let Some(inner) = &self.inner {
            serde_json::to_string(&inner.ast).unwrap_or_default()
        } else {
            "".to_owned()
        }
    }
}

#[allow(dead_code)]
struct DWParserInner {
    syn: String, //NOTE 不能修改
    ast: DWSyntax<'static>
}
