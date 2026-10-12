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
