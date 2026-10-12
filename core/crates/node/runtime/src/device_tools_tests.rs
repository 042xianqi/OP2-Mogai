#[derive(Default)]
struct RecordingDeviceToolService {
    requests: StdMutex<Vec<(String, CoreWatchRequest)>>,
    closed: Arc<AtomicBool>,
}
impl DeviceToolService for RecordingDeviceToolService {
    fn open(
        &self,
        request: CoreWatchRequest,
        origin: String,
    ) -> Result<CoreEventStream, CoreLinkError> {
        self.requests
            .lock()
            .unwrap()
            .push((origin, request.clone()));
        let (_sender, stream) = CoreEventStream::channel();
        let closed = self.closed.clone();
        Ok(stream.withOnClose(move || {
            closed.store(true, Ordering::SeqCst);
        }))
    }
}
fn deviceToolWatch(id: &str) -> CoreWatchRequest {
    CoreWatchRequest {
        requestId: operit_link::CoreRequestId(id.into()),
        target: DEVICE_TOOL_TARGET.into(),
        propertyName: "execute".into(),
        args: operit_link::toCoreValue(serde_json::json!({
            "tool_type":"default", "tool_name":"read_file_full", "parameters":{"path":"/target.txt"}
        }))
        .unwrap(),
    }
}

#[tokio::test]
async fn device_tools_route_between_real_routers_preserves_origin_and_closes_stream() {
    let _guard = routeTestGlobalLock().lock().await;
    installTestRuntimeScheduler();
    let joined = CoreSpace {
        spaceId: "device-tools-space".into(),
        spaceName: "Device tools".into(),
        spaceRevision: 2,
        members: vec!["device-a".into(), "device-b".into()],
    };
    let (source, _) = testCoreNodeRouterInJoinedSpace(
        "device-a",
        "device-b",
        "device-binding",
        "device-b",
        joined.clone(),
    );
    let (target, _) = testCoreNodeRouterInJoinedSpace(
        "device-b",
        "device-a",
        "device-binding",
        "device-b",
        joined,
    );
    let service = Arc::new(RecordingDeviceToolService::default());
    target.bindDeviceToolService(service.clone()).unwrap();
    let link = installTestPeer(
        &source,
        "device-b".into(),
        TestCoreNodeRouterEndpoint::new(target),
    )
    .unwrap();
    let stream = source
        .watchNode("device-b".into(), deviceToolWatch("device-route"))
        .await
        .unwrap();
    let recorded = service.requests.lock().unwrap();
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].0, "device-a");
    assert_eq!(recorded[0].1, deviceToolWatch("device-route"));
    drop(recorded);
    drop(stream);
    assert!(service.closed.load(Ordering::SeqCst));
    link.close();
}

#[tokio::test]
async fn device_tools_reject_missing_capability_and_binding_route_bypasses() {
    let _guard = routeTestGlobalLock().lock().await;
    installTestRuntimeScheduler();
    let router = testCoreNodeRouter("tools-target", "tools-origin", "tools-binding");
    let service = Arc::new(RecordingDeviceToolService::default());
    router.bindDeviceToolService(service.clone()).unwrap();
    router
        .networkControlStore
        .setIdentity(
            operit_store::NetworkControlStore::NetworkControlIdentityAssignment {
                nodeId: "tools-origin".into(),
                roleId: "relay".into(),
            },
        )
        .unwrap();
    let error = router
        .openDeviceToolWatch(
            deviceToolWatch("permission"),
            "tools-origin".into(),
            RoutedCoreRequestKind::Target,
        )
        .err()
        .unwrap();
    assert_eq!(error.code, "ROUTE_PERMISSION_DENIED");
    let error = router
        .openDeviceToolWatch(
            deviceToolWatch("binding"),
            "tools-target".into(),
            RoutedCoreRequestKind::SpaceBinding,
        )
        .err()
        .unwrap();
    assert_eq!(error.code, "DEVICE_TOOL_ROUTE_INVALID");
    let response = router
        .callNode(
            "tools-origin".into(),
            CoreCallRequest::new("unknown", DEVICE_TOOL_TARGET, "execute", CoreValue::Null),
        )
        .await;
    assert!(response.result.is_err());
    let response = router
        .executeLocalCall(CoreCallRequest::new(
            "call-bypass",
            DEVICE_TOOL_TARGET,
            "execute",
            CoreValue::Null,
        ))
        .await;
    assert_eq!(
        response.result.unwrap_err().code,
        "DEVICE_TOOL_MODE_INVALID"
    );
    assert!(service.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn device_tools_unreachable_device_never_executes_local_service() {
    let _guard = routeTestGlobalLock().lock().await;
    installTestRuntimeScheduler();
    let source = testCoreNodeRouter("tools-local", "tools-offline", "tools-offline-binding");
    let service = Arc::new(RecordingDeviceToolService::default());
    source.bindDeviceToolService(service.clone()).unwrap();
    let link = installTestPeer(
        &source,
        "tools-offline".into(),
        Arc::new(TestClientEndpoint),
    )
    .unwrap();
    link.close();
    assert!(source
        .watchNode("tools-offline".into(), deviceToolWatch("offline"))
        .await
        .is_err());
    assert!(service.requests.lock().unwrap().is_empty());
}

/// Connects A -> B -> C through real routers, with only the next hop paired at each source.
fn deviceToolsRelayRoute() -> (
    CoreNodeRouter,
    CoreNodeRouter,
    CoreNodeRouter,
    Arc<TestPeerService>,
    Arc<TestPeerService>,
) {
    let space = CoreSpace {
        spaceId: "tools-multihop".into(),
        spaceName: "Tool relay".into(),
        spaceRevision: 2,
        members: vec!["tools-a".into(), "tools-b".into(), "tools-c".into()],
    };
    let (source, _) = testCoreNodeRouterInJoinedSpace(
        "tools-a",
        "tools-b",
        "tools-binding",
        "tools-c",
        space.clone(),
    );
    let (relay, _) = testCoreNodeRouterInJoinedSpace(
        "tools-b",
        "tools-c",
        "tools-binding",
        "tools-c",
        space.clone(),
    );
    let (target, _) =
        testCoreNodeRouterInJoinedSpace("tools-c", "tools-b", "tools-binding", "tools-c", space);
    for router in [&source, &relay, &target] {
        router
            .networkControlStore
            .setIdentity(
                operit_store::NetworkControlStore::NetworkControlIdentityAssignment {
                    nodeId: "tools-a".into(),
                    roleId: "admin".into(),
                },
            )
            .unwrap();
        router
            .networkControlStore
            .setIdentity(
                operit_store::NetworkControlStore::NetworkControlIdentityAssignment {
                    nodeId: "tools-b".into(),
                    roleId: "relay".into(),
                },
            )
            .unwrap();
    }
    relay
        .spaceStore
        .publishLocalAdjacency(vec!["tools-c".into()], 60_000, 1000)
        .unwrap();
    // Copy the synchronized B adjacency announcement into A's topology view.
    let path = format!(
        "{}/tools-b.preferences.json",
        operit_util::RuntimeStorageLayout::RUNTIME_SPACE_TOPOLOGY_DIR_PATH
    );
    let adjacency = relay
        .localCore
        .runtimeStorageHost()
        .readBytes(&path)
        .unwrap();
    source
        .localCore
        .runtimeStorageHost()
        .writeBytes(&path, &adjacency)
        .unwrap();
    let bc = TestPeerService::new(
        "tools-b".into(),
        "tools-c".into(),
        TestCoreNodeRouterEndpoint::new(target.clone()),
    );
    relay
        .installNodeServices(NodeServices::new(bc.clone()))
        .unwrap();
    let ab = TestPeerService::new(
        "tools-a".into(),
        "tools-b".into(),
        TestCoreNodeRouterEndpoint::new(relay.clone()),
    );
    source
        .installNodeServices(NodeServices::new(ab.clone()))
        .unwrap();
    (source, relay, target, ab, bc)
}

#[derive(Default)]
struct MultiHopDeviceTools {
    origins: StdMutex<Vec<String>>,
    cancelled: Arc<AtomicBool>,
    fail_open: AtomicBool,
}
impl DeviceToolService for MultiHopDeviceTools {
    fn open(
        &self,
        request: CoreWatchRequest,
        origin: String,
    ) -> Result<CoreEventStream, CoreLinkError> {
        self.origins.lock().unwrap().push(origin);
        if self.fail_open.load(Ordering::SeqCst) {
            return Err(CoreLinkError::new(
                "PEER_RESPONSE_CLOSED",
                "Reply lost after target submission",
            ));
        }
        let (sender, stream) = CoreEventStream::channel();
        if request.args == CoreValue::Null {
            let cancelled = self.cancelled.clone();
            defaultHostRuntimeTaskSchedulerHost()
                .scheduleHostRuntimeAsyncTask(
                    "test-device-cancel",
                    Box::new(move || {
                        Box::pin(async move {
                            sender.closed().await;
                            cancelled.store(true, Ordering::SeqCst);
                        })
                    }),
                )
                .unwrap();
        } else {
            sender
                .send(operit_link::CoreEvent {
                    requestId: Some(request.requestId),
                    target: DEVICE_TOOL_TARGET.into(),
                    propertyName: request.propertyName,
                    kind: CoreEventKind::Completed,
                    value: operit_link::toCoreValue(serde_json::json!({
                        "success": true,
                        "data": {"kind": "value", "value": {"changed": "/target.txt"}},
                        "error": null,
                    }))
                    .unwrap(),
                })
                .unwrap();
        }
        Ok(stream)
    }
}

#[tokio::test]
async fn device_tools_multihop_preserves_origin_result_cancellation_and_permissions() {
    let _guard = routeTestGlobalLock().lock().await;
    installTestRuntimeScheduler();
    let (source, relay, target, ab, bc) = deviceToolsRelayRoute();
    let service = Arc::new(MultiHopDeviceTools::default());
    target.bindDeviceToolService(service.clone()).unwrap();
    let mut stream = source
        .watchNode("tools-c".into(), deviceToolWatch("multihop-change"))
        .await
        .unwrap();
    let event = stream.recv().await.unwrap();
    assert_eq!(event.kind, CoreEventKind::Completed);
    let result: serde_json::Value = operit_link::fromCoreValue(event.value).unwrap();
    assert_eq!(result["success"], true);
    assert_eq!(result["data"]["value"]["changed"], "/target.txt");
    assert_eq!(*service.origins.lock().unwrap(), vec!["tools-a"]);
    assert!(
        relay.deviceToolService.get().is_none(),
        "A relay does not execute Core tools"
    );
    let mut waiting = deviceToolWatch("multihop-cancel");
    waiting.args = CoreValue::Null;
    let stream = source.watchNode("tools-c".into(), waiting).await.unwrap();
    drop(stream);
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while !service.cancelled.load(Ordering::SeqCst) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    target
        .networkControlStore
        .setIdentity(
            operit_store::NetworkControlStore::NetworkControlIdentityAssignment {
                nodeId: "tools-a".into(),
                roleId: "relay".into(),
            },
        )
        .unwrap();
    let error = source
        .watchNode("tools-c".into(), deviceToolWatch("multihop-denied"))
        .await
        .err()
        .unwrap();
    assert_eq!(error.code, "ROUTE_PERMISSION_DENIED");
    assert_eq!(service.origins.lock().unwrap().len(), 2);
    ab.close();
    bc.close();
}

#[tokio::test]
async fn device_tools_multihop_never_resubmits_after_an_uncertain_reply() {
    let _guard = routeTestGlobalLock().lock().await;
    installTestRuntimeScheduler();
    let (source, _relay, target, ab, bc) = deviceToolsRelayRoute();
    let service = Arc::new(MultiHopDeviceTools::default());
    service.fail_open.store(true, Ordering::SeqCst);
    target.bindDeviceToolService(service.clone()).unwrap();
    let error = source
        .watchNode("tools-c".into(), deviceToolWatch("uncertain"))
        .await
        .err()
        .unwrap();
    assert_eq!(
        error.code, "PEER_RESPONSE_CLOSED",
        "Both source and relay must preserve the original uncertain outcome"
    );
    assert_eq!(service.origins.lock().unwrap().len(), 1);
    assert!(!canRetryRouteTarget("edge.plugins"));
    assert!(!canRetryRouteTarget("edge.deviceIo"));
    assert!(canRetryRouteTarget("chat"));
    ab.close();
    bc.close();
}
