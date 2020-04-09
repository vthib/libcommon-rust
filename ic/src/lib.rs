pub mod error;
pub mod ic;
pub mod msg;
pub mod types;

pub mod ic_async;

use libcommon_module::Module;
use libcommon_sys as sys;

pub fn use_module() -> Module {
    Module::new(unsafe { sys::ic_get_module() })
}
