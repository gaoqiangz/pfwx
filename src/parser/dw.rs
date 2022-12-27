use crate::retcode;
use dwparser::DWSyntax;
use pbni::{pbx::*, prelude::*};
use std::mem::transmute;

#[derive(Default)]
struct DWParser {
    inner: Option<DWParserInner>
}

#[nonvisualobject(name = "n_dwparser")]
impl DWParser {
    #[constructor]
    fn new(_session: Session, _ctx: ContextObject) -> Self { Default::default() }

    #[method]
    fn version(&self) -> String { String::from("1.0") }

    #[method]
    fn copyright(&self) -> String { String::from(env!("CARGO_PKG_AUTHORS")) }

    /// 从DW语法解析
    ///
    /// 支持`.srd`文件格式
    #[method]
    fn parse(&mut self, syn: String) -> pblong {
        let syn_ref: &'static str = unsafe {
            //SAFETY
            transmute(syn.as_str())
        };
        let ast = match DWSyntax::parse(syn_ref) {
            Ok(ast) => ast,
            Err(_) => return retcode::E_INVALID_ARGUMENT
        };
        self.inner = Some(DWParserInner {
            syn,
            ast
        });
        retcode::OK
    }

    /// 获取指定语法项的参数值
    ///
    /// 兼容`DataWindow::Describe`参数和返回值
    #[method]
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
    #[method]
    fn modify(&mut self, modifier: String) -> String {
        if let Some(inner) = &mut self.inner {
            inner.ast.modify(&modifier)
        } else {
            "!".to_owned()
        }
    }

    /// 反序列化`JSON-AST`字符串
    #[method]
    fn from_json_ast(&mut self, syn: String) -> pblong {
        let syn_ref: &'static str = unsafe {
            //SAFETY
            transmute(syn.as_str())
        };
        let ast = match serde_json::from_str::<DWSyntax>(syn_ref) {
            Ok(ast) => ast,
            Err(_) => return retcode::E_INVALID_ARGUMENT
        };
        self.inner = Some(DWParserInner {
            syn,
            ast
        });
        retcode::OK
    }

    /// 序列化为`JSON-AST`字符串
    #[method]
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
