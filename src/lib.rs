#![windows_subsystem = "windows"]
#![allow(dead_code)]
#![feature(try_trait_v2)]

#[cfg(feature = "trace")]
#[macro_use]
extern crate tracing;

mod base;
mod pbx;
#[cfg(feature = "reactor")]
mod reactor;

mod prelude {
    pub(crate) use super::base::retcode::RetCode;
    #[cfg(feature = "reactor")]
    pub(crate) use super::reactor;
}
