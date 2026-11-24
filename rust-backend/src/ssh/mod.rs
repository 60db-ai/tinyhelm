//! Remote execution on managed servers (port of `remoteProcess.php`, `sudo.php`, `SshMultiplexingHelper`).

pub mod remote;
pub mod sudo;

#[allow(unused_imports)]
pub use remote::{instant_remote_process, RemoteError, RunOptions, ServerConnection};
