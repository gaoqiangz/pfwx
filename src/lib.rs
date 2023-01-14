#![windows_subsystem = "windows"]
#![allow(dead_code)]
#![feature(try_trait_v2)]

mod base;

#[cfg(feature = "reactor")]
mod reactor;
#[cfg(feature = "http")]
mod http;
#[cfg(feature = "mqtt")]
mod mqtt;
#[cfg(feature = "parser")]
mod parser;

mod prelude {
    pub(crate) use super::base::retcode::RetCode;
    #[cfg(feature = "reactor")]
    pub(crate) use super::reactor;
}
