//! Run the `Service` as an HTTP endpoint.

mod client;
mod server;

pub use self::client::*;
pub use self::server::HttpApp;
pub(crate) use self::server::*;