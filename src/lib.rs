#![allow(dead_code)]
#![feature(try_trait_v2)]

mod base;

#[cfg(feature = "reactor")]
mod reactor;
#[cfg(feature = "parser")]
mod parser;
#[cfg(feature = "http")]
mod http;

mod prelude {
    pub(crate) use super::{base::retcode::RetCode, reactor};
}
