use crate::retcode;
use pbni::{pbx::*, prelude::*};

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

    #[method]
    fn parse(&mut self, syn: String) -> pblong {
        //SAFETY
        //自引用与实例生命期一样长并且不会暴露在外部所以是安全的
        let syn_ref: &'static str = unsafe { std::mem::transmute(syn.as_str()) };
        let ast = match dwparser::DWSyntax::parse(syn_ref) {
            Ok(ast) => ast,
            Err(_) => return retcode::E_INVALID_ARGUMENT
        };
        self.inner = Some(DWParserInner {
            syn,
            ast
        });
        retcode::OK
    }

    #[method]
    fn describe(&self, selector: String) -> String {
        if let Some(inner) = &self.inner {
            inner.ast.describe(&selector)
        } else {
            "!".to_owned()
        }
    }

    #[method]
    fn modify(&mut self, modifier: String) -> String {
        if let Some(inner) = &mut self.inner {
            inner.ast.modify(&modifier)
        } else {
            "empty".to_owned()
        }
    }
}

#[allow(dead_code)]
struct DWParserInner {
    syn: String,
    ast: dwparser::DWSyntax<'static>
}
