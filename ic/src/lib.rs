pub mod error;
pub mod ic;
pub mod ic_sync;
pub mod msg_sync;
pub mod types;
pub mod types_sync;

use libcommon_module::Module;
use libcommon_sys as sys;

pub fn use_module() -> Module {
    Module::new(unsafe { sys::ic_get_module() })
}
