use std::error;
use std::fmt;
use libcommon_sys as sys;

#[derive(Debug)]
pub enum Error {
    Generic(String),
    Retry,
    Abort,
    Invalid,
    Unimplemented,
    ServerError,
    ProxyError,
    TimedOut,
    Canceled
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "query error: {}", match self {
            Error::Generic(s) => s,
            Error::Retry => "retry",
            Error::Abort => "abort",
            Error::Invalid => "invalid",
            Error::Unimplemented => "unimplemented",
            Error::ServerError => "server error",
            Error::ProxyError => "proxy error",
            Error::TimedOut => "timed out",
            Error::Canceled => "canceled"
        })
    }
}

impl error::Error for Error {
}

impl From<sys::ic_status_t> for Error {
    fn from(status: sys::ic_status_t) -> Self {
        match status {
            /* TODO: handle exception */
            sys::ic_status_t_IC_MSG_EXN => Self::Generic("exception".to_owned()),
            sys::ic_status_t_IC_MSG_RETRY => Self::Retry,
            sys::ic_status_t_IC_MSG_ABORT => Self::Abort,
            sys::ic_status_t_IC_MSG_INVALID => Self::Invalid,
            sys::ic_status_t_IC_MSG_UNIMPLEMENTED => Self::Unimplemented,
            sys::ic_status_t_IC_MSG_SERVER_ERROR => Self::ServerError,
            sys::ic_status_t_IC_MSG_PROXY_ERROR => Self::ProxyError,
            sys::ic_status_t_IC_MSG_TIMEDOUT => Self::TimedOut,
            sys::ic_status_t_IC_MSG_CANCELED => Self::Canceled,
            _ => {
                unreachable!();
            },
        }
    }
}
