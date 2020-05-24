use crate::error;
use crate::ic::{Channel, QueryFuture, RpcRegister};
use futures::future::Future;
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

    fn implement<F, Fut>(reg: &mut RpcRegister, iface_tag: u16, fun: F)
    where
        F: Fn(Channel, Self::Input) -> Fut + 'static,
        Fut: Future<Output = Result<Self::Output, error::Error>> + 'static,
        Self::Output: 'static,
    {
        reg.register(Self::get_cmd(iface_tag), fun);
    }

    fn call(ic: &mut Channel, iface_tag: u16, arg: Self::Input) -> QueryFuture<Self::Output> {
        let input = to_bytes(&arg).unwrap();

        QueryFuture::new(ic, &input, Self::get_cmd(iface_tag), Self::ASYNC)
    }
}
