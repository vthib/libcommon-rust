use crate::error;
use crate::ic_async::{Channel, QueryFuture, RpcRegister};
use serde_iop::{Deserialize, Serialize};

pub trait Rpc {
    type Input: Serialize + Deserialize<'static>;
    type Output: Serialize + Deserialize<'static>;
}

pub trait ModRpc {
    type RPC: Rpc;

    const ASYNC: bool;
    const CMD: i32;

    fn implement<F>(reg: &mut RpcRegister<'static>, fun: F)
    where
        F: Fn(<Self::RPC as Rpc>::Input) -> Result<<Self::RPC as Rpc>::Output, error::Error>
            + 'static,
    {
        reg.register(Self::CMD, fun);
    }

    fn call(ic: &mut Channel, arg: <Self::RPC as Rpc>::Input) -> QueryFuture<Self::RPC> {
        QueryFuture::new(ic, arg, Self::CMD, Self::ASYNC)
    }
}
