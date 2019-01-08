pub mod client;
pub mod server;

pub use self::client::Rpc;
pub use self::server::RpcHandler;

use failure::Fail;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Default, Deserialize, Serialize, Fail, Debug)]
#[fail(display = "rpc error found")]
pub struct RpcError {}

thread_local!(
    /// Each thread maintains its own `App` struct, which is basically
    /// routing information for messages
    pub(crate) static SOCKET_DIR: tempfile::TempDir = tempfile::tempdir().unwrap();
);

pub(crate) fn sock_path(name: &str) -> PathBuf {
	// let tmp_dir = tempdir::TempDir::new("actix-dir").unwrap();
	SOCKET_DIR.with(|dir| dir.path().join(format!("{}.sock", name)))
}