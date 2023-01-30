use pbni::pbx::{pbobject, pbsession, Object, Session};
use std::slice;

lazy_static::lazy_static! {
static ref API: &'static Api = unsafe { Api::load() };
}

/// 解析`pfw::n_json`对象
pub fn json_parse<'a>(session: Session, data: &str) -> Object<'a> {
    unsafe {
        let obj = (API.JsonParseUTF8)(session.as_raw(), data.as_ptr(), data.len());
        Object::from_raw(obj, session)
    }
}

/// 序列化`pfw::n_json`对象
pub fn json_serialize(obj: &Object) -> String {
    unsafe {
        let mut len = 0;
        let buf = (API.JsonSerializeUTF8)(obj.get_session().as_raw(), obj.as_raw(), &mut len as _);
        if !buf.is_null() {
            let buf_slice = slice::from_raw_parts(buf, len);
            let rv = String::from(std::str::from_utf8_unchecked(buf_slice));
            (API.Free)(buf);
            rv
        } else {
            "".to_owned()
        }
    }
}

/// 解析`pfw::n_xmldoc`对象
pub fn xml_parse<'a>(session: Session, data: &str) -> Object<'a> {
    unsafe {
        let obj = (API.XmlParseUTF8)(session.as_raw(), data.as_ptr(), data.len());
        Object::from_raw(obj, session)
    }
}

/// 序列化`pfw::n_xmldoc`对象
pub fn xml_serialize(obj: &Object) -> String {
    unsafe {
        let mut len = 0;
        let buf = (API.XmlSerializeUTF8)(obj.get_session().as_raw(), obj.as_raw(), &mut len as _);
        if !buf.is_null() {
            let buf_slice = slice::from_raw_parts(buf, len);
            let rv = String::from(std::str::from_utf8_unchecked(buf_slice));
            (API.Free)(buf);
            rv
        } else {
            "".to_owned()
        }
    }
}

#[allow(non_snake_case)]
#[repr(C)]
struct Api {
    JsonParseUTF8: extern "system" fn(pbsession: pbsession, data: *const u8, len: usize) -> pbobject,
    JsonSerializeUTF8:
        extern "system" fn(pbsession: pbsession, pbobject: pbobject, len: *mut usize) -> *mut u8,
    XmlParseUTF8: extern "system" fn(pbsession: pbsession, data: *const u8, len: usize) -> pbobject,
    XmlSerializeUTF8:
        extern "system" fn(pbsession: pbsession, pbobject: pbobject, len: *mut usize) -> *mut u8,
    Free: extern "system" fn(data: *mut u8)
}

impl Api {
    unsafe fn load() -> &'static Api {
        type GetApiFn = extern "system" fn() -> *const Api;
        let lib = libloading::Library::new("pfw.dll").expect("Cannot load module pfw.dll");
        let api_fn = lib.get::<GetApiFn>(b"pfwAPI").expect("Cannot find entry symbol 'pfwAPI' at pfw.dll");
        &*api_fn()
    }
}
