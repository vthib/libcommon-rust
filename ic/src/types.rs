use serde_iop::{Deserialize, Serialize};

pub trait Rpc {
    type Input: Serialize + Deserialize<'static>;
    type Output: Serialize + Deserialize<'static>;
}

pub trait ModRpc {
    type RPC: Rpc;

    const ASYNC: bool;
    const CMD: i32;
}
