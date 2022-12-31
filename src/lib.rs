#![feature(try_trait_v2)]

mod retcode;
#[cfg(feature = "reactor")]
mod reactor;
#[cfg(feature = "parser")]
mod parser;
#[cfg(feature = "http")]
mod http;

mod prelude {
    pub(crate) use super::{reactor, retcode::RetCode};
}
