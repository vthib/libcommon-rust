mod de;
mod error;
mod ser;
mod wire;

pub use de::from_bytes;
pub use ser::to_bytes;

pub use serde::de::DeserializeOwned;
pub use serde::{Deserialize, Serialize};
