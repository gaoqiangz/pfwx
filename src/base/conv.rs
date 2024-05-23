use pbni::primitive::pblong;
use std::borrow::Cow;

pub const ENCODING_UNKNOWN: pblong = 0;
pub const ENCODING_UTF8: pblong = 1;
pub const ENCODING_UTF16: pblong = 2;
pub const ENCODING_UTF16LE: pblong = 2;
pub const ENCODING_UTF16BE: pblong = 3;
pub const ENCODING_ANSI: pblong = 4;
pub const ENCODING_GB2312: pblong = 5;
pub const ENCODING_GBK: pblong = 5;
pub const ENCODING_GB18030: pblong = 6;
pub const ENCODING_BIG5: pblong = 7;
pub const ENCODING_ISO88591: pblong = 8;
pub const ENCODING_LATIN1: pblong = 8;
pub const ENCODING_ISO88592: pblong = 9;
pub const ENCODING_LATIN2: pblong = 9;
pub const ENCODING_ISO88593: pblong = 10;
pub const ENCODING_LATIN3: pblong = 10;
pub const ENCODING_ISO2022JP: pblong = 11;
pub const ENCODING_ISO2022KR: pblong = 12;

fn codepage(encoding: pblong) -> usize {
    match encoding {
        ENCODING_ANSI => 0,
        ENCODING_UTF8 => 65001,
        ENCODING_UTF16LE => 1200,
        ENCODING_UTF16BE => 1201,
        ENCODING_GB2312 => 936,
        ENCODING_GB18030 => 54936,
        ENCODING_BIG5 => 950,
        ENCODING_ISO88591 => 28591,
        ENCODING_ISO88592 => 28592,
        ENCODING_ISO88593 => 28593,
        ENCODING_ISO2022JP => 50220,
        ENCODING_ISO2022KR => 50225,
        _ => 0
    }
}

/// 通过指定编码进行字符串编码
///
/// NOTE 默认`utf-8`
#[cfg(feature = "encoding")]
pub fn encode(data: &str, encoding: pblong) -> Cow<[u8]> {
    let codec =
        encoding::label::encoding_from_windows_code_page(codepage(encoding)).unwrap_or(encoding::all::UTF_8);
    if codec.name() == "utf-8" {
        Cow::Borrowed(data.as_bytes())
    } else {
        codec.encode(data, encoding::EncoderTrap::Replace).map(Cow::from).unwrap_or_default()
    }
}

/// 通过指定编码进行字符串解码
///
/// NOTE 默认`utf-8`
#[cfg(feature = "encoding")]
pub fn decode(data: &[u8], encoding: pblong) -> Cow<str> {
    let codec =
        encoding::label::encoding_from_windows_code_page(codepage(encoding)).unwrap_or(encoding::all::UTF_8);
    if codec.name() == "utf-8" {
        String::from_utf8_lossy(&data)
    } else {
        codec.decode(&data, encoding::DecoderTrap::Replace).map(Cow::from).unwrap_or_default()
    }
}

/// 通过指定字符集名称进行字符串解码
///
/// NOTE 默认`utf-8`
#[cfg(feature = "encoding")]
pub fn decode_by_charset<'a>(data: &'a [u8], charset: &str) -> Cow<'a, str> {
    let codec = encoding::label::encoding_from_whatwg_label(charset).unwrap_or(encoding::all::UTF_8);
    if codec.name() == "utf-8" {
        String::from_utf8_lossy(&data)
    } else {
        codec.decode(&data, encoding::DecoderTrap::Replace).map(Cow::from).unwrap_or_default()
    }
}
