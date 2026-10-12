//! Edge startup selects code confirmation without a listener token.
//! Session encryption and Space authorization remain in the shared runtime.
use std::sync::Arc;

use operit_host_api::HostManager::HostManager;
use operit_link::protocol::LinkDeviceInfo;
use operit_node_runtime::HostRuntimePeerService::{
    HostRuntimePeerService, PeerPairingPolicy, PeerRuntimeLimits,
};
use operit_node_runtime::PeerRouter::PeerRouter;

/// Creates the Edge receiver using the existing pairing and session engine.
/// The policy belongs to this local node, independent of peer metadata or OS.
#[allow(non_snake_case)]
pub fn createPeerService<R: PeerRouter + 'static>(
    host: Arc<HostManager>,
    router: &Arc<R>,
    info: LinkDeviceInfo,
    limits: PeerRuntimeLimits,
) -> Result<Arc<HostRuntimePeerService>, String> {
    HostRuntimePeerService::newWithPairingPolicy(
        host,
        router,
        info,
        limits,
        PeerPairingPolicy::ConfirmationCodeOnly,
    )
}
