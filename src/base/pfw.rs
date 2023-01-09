use pbni::pbx::Object;
use std::slice;

lazy_static::lazy_static! {
static ref API: &'static Api = unsafe { Api::load().expect("can't load pfw.dll") };
}

/// 序列化`pfw::n_json`对象
pub fn json_serialize(obj: &Object) -> String {
    unsafe {
        let mut len = 0;
        let buf = (API.JsonSerializeUTF8)(
            obj.get_session().as_raw().as_ptr() as _,
            obj.as_raw().as_ptr() as _,
            &mut len as _
        );
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

/// 序列化`pfw::n_xmldoc`对象
pub fn xml_serialize(obj: &Object) -> String {
    unsafe {
        let mut len = 0;
        let buf = (API.XmlSerializeUTF8)(
            obj.get_session().as_raw().as_ptr() as _,
            obj.as_raw().as_ptr() as _,
            &mut len as _
        );
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
    JsonSerializeUTF8: extern "system" fn(*const (), *const (), *mut usize) -> *mut u8,
    XmlSerializeUTF8: extern "system" fn(*const (), *const (), *mut usize) -> *mut u8,
    Free: extern "system" fn(*mut u8)
}

impl Api {
    unsafe fn load() -> Result<&'static Api, libloading::Error> {
        type GetApiFn = extern "system" fn() -> *const Api;
        let lib = libloading::Library::new("pfw.dll")?;
        let api_fn = lib.get::<GetApiFn>(b"pfwAPI")?;
        Ok(&*api_fn())
    }
}
