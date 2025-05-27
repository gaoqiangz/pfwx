#![allow(dead_code)]
#![allow(non_camel_case_types)]

use std::{convert::Infallible, ops::FromResidual};

use pbni::pbx::{FromValue, Result as PBXResult, ToValue, Value, PBXRESULT};

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetCode {
    OK = 0,
    PREVENT = 1,
    FAILED = -1,
    CANCELLED = -2,
    E_INVALID_ARGUMENT = -3,
    E_INVALID_IMAGE = -4,
    E_INVALID_OBJECT = -5,
    E_INVALID_TYPE = -6,
    E_INVALID_TRANSACTION = -7,
    E_INVALID_SQL = -8,
    E_INVALID_DATA = -9,
    E_INVALID_DATAOBJECT = -10,
    E_INVALID_HANDLE = -11,
    E_OUT_OF_BOUND = -12,
    E_OUT_OF_RANGE = -13,
    E_OUT_OF_MEMORY = -14,
    E_FILE_NOT_FOUND = -15,
    E_OBJECT_NOT_FOUND = -16,
    E_DATA_NOT_FOUND = -17,
    E_FUNCTION_NOT_FOUND = -18,
    E_EVENT_NOT_FOUND = -19,
    E_MEMBER_NOT_FOUND = -20,
    E_VAR_NOT_FOUND = -21,
    E_NOT_EXISTS = -22,
    E_BUSY = -23,
    E_TIME_OUT = -24,
    E_ACCESS_DENIED = -25,
    E_WIN32_ERROR = -26,
    E_INTERNAL_ERROR = -27,
    E_DB_ERROR = -28,
    E_HTTP_ERROR = -29,
    E_WINHTTP_ERROR = -30,
    E_IO_ERROR = -31,
    E_SQL_BIND_ARG_FAILED = -32,
    E_RETRY = -33,
    E_NO_SUPPORT = -2000,
    E_NO_IMPLEMENTATION = -2001,
    UNKNOWN = -4000
}

impl FromValue<'_> for RetCode {
    fn from_value(val: Option<Value>) -> PBXResult<Self> {
        if let Some(val) = val {
            val.try_get_long().map(|val| unsafe { std::mem::transmute(val.unwrap_or_default()) })
        } else {
            Err(PBXRESULT::E_INVOKE_WRONG_NUM_ARGS)
        }
    }
    unsafe fn from_value_unchecked(val: Option<Value>) -> PBXResult<Self> {
        if let Some(val) = val {
            Ok(std::mem::transmute(val.get_long_unchecked().unwrap_or_default()))
        } else {
            Err(PBXRESULT::E_INVOKE_WRONG_NUM_ARGS)
        }
    }
}

impl ToValue for RetCode {
    fn to_value(self, val: &mut Value) -> PBXResult<()> { val.try_set_long(self as _) }
    unsafe fn to_value_unchecked(self, val: &mut Value) -> PBXResult<()> {
        val.set_long_unchecked(self as _);
        Ok(())
    }
}

impl<E> FromResidual<Result<Infallible, E>> for RetCode {
    fn from_residual(residual: Result<Infallible, E>) -> Self {
        match residual {
            Ok(_) => unreachable!(),
            Err(_) => RetCode::FAILED
        }
    }
}

impl<T, E> From<Result<T, E>> for RetCode {
    fn from(res: Result<T, E>) -> Self {
        if res.is_ok() {
            RetCode::OK
        } else {
            RetCode::FAILED
        }
    }
}
