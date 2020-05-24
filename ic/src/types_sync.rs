use crate::error;
use crate::ic_sync::{Channel, RpcRegister};
use crate::msg_sync::Msg;
use serde_iop::to_bytes;
use serde_iop::{DeserializeOwned, Serialize};

pub trait Rpc {
    type Input: Serialize + DeserializeOwned;
    type Output: Serialize + DeserializeOwned;

    const TAG: u16;
    const ASYNC: bool;

    fn get_cmd(iface_tag: u16) -> i32 {
        ((iface_tag as i32) << 16) | (Self::TAG as i32)
    }

    fn implement<F>(reg: &mut RpcRegister, iface_tag: u16, fun: F)
    where
        F: Fn(Self::Input) -> Result<Self::Output, error::Error> + 'static,
    {
        reg.register(Self::get_cmd(iface_tag), fun);
    }

    fn call<F>(ic: &mut Channel, iface_tag: u16, arg: Self::Input, cb: F)
    where
        F: FnOnce(&mut Channel, Result<Self::Output, error::Error>) + 'static,
    {
        let input = to_bytes(&arg).unwrap();

        let msg = Msg::new(&input, Self::get_cmd(iface_tag), Self::ASYNC, cb);
        msg.send(ic);
    }
}
