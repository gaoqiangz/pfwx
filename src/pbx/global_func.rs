use crate::prelude::*;
use pbni::pbx::*;

#[global_function(name = "pfwxFinalize")]
fn finalize() {
    //销毁运行时
    #[cfg(feature = "reactor")]
    reactor::runtime::shutdown();
}
