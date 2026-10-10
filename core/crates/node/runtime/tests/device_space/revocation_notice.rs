use super::*;

/// Publishes every device's live adjacency exactly like a sync round does,
/// then carries the records the way the Space snapshot exchange does.
fn publishRevocationFixtureAdjacency(routers: &[CoreNodeRouter], peers: &[Arc<ApprovalMeshPeer>]) {
    for (router, peer) in routers.iter().zip(peers) {
        let activePeers = peer.activePeerNodeIds().unwrap();
        crate::NodeSpaceService::space_topology::publishLocalTopology(
            &router.spaceStore,
            &router.networkControlStore,
            &router.localNodeId(),
            &activePeers,
        )
        .expect("production adjacency publication must succeed");
    }
    let topology = routers
        .iter()
        .map(|router| {
            router
                .spaceStore
                .topologyRecords()
                .unwrap()
                .remove(&router.localNodeId())
                .expect("every device publishes its own adjacency")
        })
        .collect::<Vec<_>>();
    for router in routers {
        router
            .spaceStore
            .importTopologyRecords(topology.clone())
            .unwrap();
    }
}

/// Builds one authorized Space over a mesh of `nodeCount` routers with the
/// listed direct links and published adjacency.
fn revocationMesh(
    prefix: &str,
    nodeCount: usize,
    links: &[(usize, usize)],
) -> (
    Vec<CoreNodeRouter>,
    Vec<RuntimeRemoteLinkService>,
    Vec<Arc<ApprovalMeshPeer>>,
) {
    let services = (0..nodeCount)
        .map(|index| approvalService(&format!("{prefix}-{index}")))
        .collect::<Vec<_>>();
    let routers = services
        .iter()
        .map(|(router, _)| router.clone())
        .collect::<Vec<_>>();
    let access = services
        .iter()
        .map(|(_, service)| service.clone())
        .collect::<Vec<_>>();
    mergedChainFixture(&routers.iter().collect::<Vec<_>>());
    let peers = routers
        .iter()
        .map(|router| ApprovalMeshPeer::new(router.localNodeId()))
        .collect::<Vec<_>>();
    for (a, b) in links {
        peers[*a].link(&routers[*b]);
        peers[*b].link(&routers[*a]);
    }
    for (router, peer) in routers.iter().zip(&peers) {
        router
            .installNodeServices(NodeServices::new(peer.clone()))
            .unwrap();
    }
    publishRevocationFixtureAdjacency(&routers, &peers);
    (routers, access, peers)
}

/// Verifies one online removal reaches the removed device, which retires
/// itself against its own replayed log, and that completing the retreat drops
/// every pairing credential: mutual forgetting.
#[tokio::test]
async fn direct_removal_retires_the_removed_device_and_drops_every_pairing() {
    let _guard = routeTestGlobalLock().lock().await;
    installTestRuntimeScheduler();
    let (routers, access, peers) = revocationMesh("revoke-direct", 2, &[(0, 1)]);
    let removed = routers[1].localNodeId();

    access[0]
        .removeDeviceSpaceMember(removed.clone())
        .await
        .unwrap();

    // The remover forgets the device entirely, exactly as before this change.
    assert!(!routers[0].spaceStore.contains(removed.clone()).unwrap());
    assert!(!routers[0]
        .networkControlStore
        .currentState()
        .unwrap()
        .memberNodeIds
        .contains(&removed));
    assert!(!peers[0]
        .pairedPeers()
        .unwrap()
        .iter()
        .any(|peer| peer.nodeId == removed));

    // The notice retired the removed device: it stands as its own Space and
    // keeps no membership claim on the Space it was ejected from.
    let retiredSpace = routers[1].spaceStore.space().unwrap();
    assert_eq!(retiredSpace.members, vec![removed.clone()]);
    assert!(routers[1]
        .networkControlStore
        .currentState()
        .unwrap()
        .memberNodeIds
        .contains(&removed));

    // Dropping pairings needs the peer service, so the asynchronous sweep
    // finishes it; the device keeps its pairing until that sweep runs.
    assert!(peers[1]
        .pairedPeers()
        .unwrap()
        .iter()
        .any(|peer| peer.nodeId == routers[0].localNodeId()));
    let retired = RuntimeRemoteLinkService::newWithRouter(
        (*routers[1].localCore).clone(),
        routers[1].clone(),
    );
    retired.retireRemovedDevice().await.unwrap();
    assert!(peers[1].pairedPeers().unwrap().is_empty());
    // The sweep is idempotent.
    retired.retireRemovedDevice().await.unwrap();
    assert!(peers[1].pairedPeers().unwrap().is_empty());
}

/// Verifies removing a bridge still reaches the far side: the notice travels
/// along the route captured before the removal, so the leaf learns the bridge
/// is gone instead of staying in a stale view.
#[tokio::test]
async fn bridge_removal_reaches_the_far_side_through_the_captured_route() {
    let _guard = routeTestGlobalLock().lock().await;
    installTestRuntimeScheduler();
    let (routers, access, _peers) = revocationMesh("revoke-bridge", 3, &[(0, 1), (1, 2)]);
    let bridge = routers[1].localNodeId();
    let leaf = routers[2].localNodeId();

    access[0]
        .removeDeviceSpaceMember(bridge.clone())
        .await
        .unwrap();

    // The far side applied the removal: its policy no longer admits the
    // bridge, and its own member table drops the bridge with it.
    let leafState = routers[2].networkControlStore.currentState().unwrap();
    assert!(!leafState.memberNodeIds.contains(&bridge));
    assert!(leafState.memberNodeIds.contains(&leaf));
    assert!(!routers[2].spaceStore.contains(bridge.clone()).unwrap());
    assert!(!routers[2]
        .spaceStore
        .space()
        .unwrap()
        .members
        .contains(&bridge));
    // The bridge itself retired against its own replayed log.
    assert_eq!(routers[1].spaceStore.space().unwrap().members, vec![bridge.clone()]);

    // The far side's deferred sweep finishes the mutual forgetting.
    let leafSync = crate::SpacePersistenceSyncService::SpacePersistenceSyncService::new(
        routers[2].localCore.clone(),
        routers[2].clone(),
        routers[2].spaceStore.clone(),
    );
    leafSync.forgetPendingRemovedDevices().await.unwrap();
    assert!(!routers[2].spaceStore.contains(bridge).unwrap());
}

/// Verifies a member forgets a removed device at the transition it observes,
/// and keeps pairing with devices that were never removed.
#[tokio::test]
async fn members_forget_a_removed_device_after_applying_the_removal() {
    let _guard = routeTestGlobalLock().lock().await;
    installTestRuntimeScheduler();
    let (routers, access, peers) =
        revocationMesh("revoke-member", 4, &[(0, 1), (0, 2), (0, 3), (1, 2), (2, 3)]);
    let removed = routers[3].localNodeId();

    access[0]
        .removeDeviceSpaceMember(removed.clone())
        .await
        .unwrap();

    // The observer has not synchronized yet: it still admits the removed
    // device and still holds its pairing.
    assert!(routers[2]
        .networkControlStore
        .currentState()
        .unwrap()
        .memberNodeIds
        .contains(&removed));
    assert!(peers[2]
        .pairedPeers()
        .unwrap()
        .iter()
        .any(|peer| peer.nodeId == removed));

    // One control exchange with the remover applies the removal: the policy
    // drops the device, the observer's member table shrinks with it, and the
    // deferred sweep drops the pairing while its other pairings stay.
    let sync = crate::SpacePersistenceSyncService::SpacePersistenceSyncService::new(
        routers[2].localCore.clone(),
        routers[2].clone(),
        routers[2].spaceStore.clone(),
    );
    sync.synchronizeReachablePeer(routers[0].localNodeId(), 512, false)
        .await
        .unwrap();
    assert!(!routers[2]
        .networkControlStore
        .currentState()
        .unwrap()
        .memberNodeIds
        .contains(&removed));
    assert!(!routers[2].spaceStore.contains(removed.clone()).unwrap());
    assert!(!routers[2]
        .spaceStore
        .space()
        .unwrap()
        .members
        .contains(&removed));
    sync.forgetPendingRemovedDevices().await.unwrap();
    assert!(!peers[2]
        .pairedPeers()
        .unwrap()
        .iter()
        .any(|peer| peer.nodeId == removed));
    // Pairings with devices that were never removed stay.
    assert!(peers[2]
        .pairedPeers()
        .unwrap()
        .iter()
        .any(|peer| peer.nodeId == routers[0].localNodeId()));
    assert!(peers[2]
        .pairedPeers()
        .unwrap()
        .iter()
        .any(|peer| peer.nodeId == routers[1].localNodeId()));
}

/// Verifies the notice stays best-effort: an offline target never blocks the
/// removal and never changes what the remover forgets.
#[tokio::test]
async fn removal_notice_is_best_effort_when_the_target_is_offline() {
    let _guard = routeTestGlobalLock().lock().await;
    installTestRuntimeScheduler();
    let (routers, access, peers) = revocationMesh("revoke-offline", 2, &[(0, 1)]);
    let removed = routers[1].localNodeId();
    peers[0].disconnectPeer(&removed).await.unwrap();

    access[0]
        .removeDeviceSpaceMember(removed.clone())
        .await
        .unwrap();

    assert!(!routers[0].spaceStore.contains(removed.clone()).unwrap());
    assert!(!peers[0]
        .pairedPeers()
        .unwrap()
        .iter()
        .any(|peer| peer.nodeId == removed));
    // The offline device keeps its stale view: delivery is best-effort, and a
    // partitioned device converges when a path returns.
    assert!(routers[1]
        .networkControlStore
        .currentState()
        .unwrap()
        .memberNodeIds
        .contains(&removed));
}

/// Verifies a member cannot relay another issuer's authority command: only
/// the command's own origin (or a wildcard authority) may introduce it, so a
/// removed device can never broadcast a removal it did not receive.
#[tokio::test]
async fn control_exchange_rejects_relayed_foreign_authority_commands() {
    let _guard = routeTestGlobalLock().lock().await;
    installTestRuntimeScheduler();
    let (routers, _access, _peers) =
        revocationMesh("revoke-relay", 3, &[(0, 1), (0, 2), (1, 2)]);
    let courier = routers[1].localNodeId();
    let observer = RuntimeRemoteLinkService::newWithRouter(
        (*routers[2].localCore).clone(),
        routers[2].clone(),
    );

    // The courier holds an authority command the observer has never seen and
    // tries to introduce it as its own.
    routers[0]
        .networkControlStore
        .admitMember("revoke-relay-outsider".into())
        .unwrap();
    let foreignOperations = routers[0]
        .networkControlStore
        .currentSpaceOperations()
        .unwrap();
    assert!(!foreignOperations.is_empty());
    let request = operit_link::CoreCallRequest::new(
        "relayed-authority".to_string(),
        crate::NodeSpaceService::NODE_SPACE_CONTROL_TARGET,
        "exchange",
        operit_link::toCoreValue(foreignOperations).unwrap(),
    );
    let error = crate::NodeSpaceService::acceptSpaceControlCall(&observer, &courier, request)
        .expect_err("a member must not introduce another issuer's authority command");
    assert!(error.contains("cannot introduce another issuer"), "{error}");
}
