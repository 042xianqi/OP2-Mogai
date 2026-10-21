//! Native host lifecycle execution and browser adapters for the shared Flutter bridge.
//! This module is the only place where the ABI execution style is selected.

use std::future::Future;
use std::sync::mpsc;

use operit_link::CoreLinkError;
#[cfg(target_arch = "wasm32")]
use operit_link::{CoreCallRequest, CoreCallResponse, CorePushItem, CoreWatchRequest};

#[cfg(target_arch = "wasm32")]
use crate::BridgeCodec::native_watch_event_vec;
use crate::OperitFlutterBridge;

#[cfg(target_arch = "wasm32")]
use js_sys::Function;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsValue;

/// Owns lifecycle hooks selected at the host boundary.
pub(crate) struct PlatformBridgeState;

impl PlatformBridgeState {
    /// Creates the lifecycle state used by the selected host.
    pub(crate) fn new() -> Self {
        Self
    }

    /// Stops native peer services before Core destruction.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn beforeShutdown(&self, bridge: &OperitFlutterBridge) {
        if let Ok(application) = bridge.coreApplication.lock() {
            if let Some(application) = application.as_ref() {
                if let Ok(services) = application.nodeServices() {
                    if let Err(error) = bridge
                        .runHostRuntimeAsyncTask("operit-flutter-node-stop", move || async move {
                            services.peers().stop().await
                        })
                    {
                        operit_util::AppLogger::AppLogger::e("OperitFlutterBridge", &error);
                    }
                }
            }
        }
    }

    /// Leaves browser task cancellation to the shared subscription and Core owners.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn beforeShutdown(&self, _bridge: &OperitFlutterBridge) {}

    /// Releases registrations owned by the selected host boundary.
    pub(crate) fn releaseHost(&self) {
        crate::PlatformRuntimeFactory::release_host();
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl OperitFlutterBridge {
    /// Executes a prepared shared task on this runtime's required scheduler host.
    pub(crate) fn runHostRuntimeAsyncTask<T, F>(
        &self,
        name: &'static str,
        task: impl FnOnce() -> F + Send + 'static,
    ) -> Result<T, String>
    where
        T: Send + 'static,
        F: Future<Output = T> + 'static,
    {
        let (sender, receiver) = mpsc::channel();
        self.taskScheduler
            .scheduleHostRuntimeAsyncTask(
                name,
                Box::new(move || {
                    Box::pin(async move {
                        use futures_util::FutureExt as _;
                        let outcome = std::panic::AssertUnwindSafe(task()).catch_unwind().await;
                        let result = outcome.map_err(|payload| {
                            format!(
                                "FATAL_CORE_PANIC: {}",
                                crate::BridgeExports::panic_payload_message(payload.as_ref())
                            )
                        });
                        let _ = sender.send(result);
                    })
                }),
            )
            .map_err(|error| error.to_string())?;
        receiver
            .recv()
            .map_err(|error| format!("Runtime task result channel closed: {error}"))?
    }

    /// Executes owner event ingress on the runtime's scheduler for C/JNI callers.
    pub(crate) fn emitRuntimeEvent(&self, encoded: &str) -> String {
        let result =
            crate::PlatformRuntimeFactory::runtimeEventRequest(encoded).and_then(|request| {
                let task = self.prepareCall(request);
                self.runHostRuntimeAsyncTask("operit-flutter-runtime-event", move || task.execute())
                    .map_err(CoreLinkError::internal)?
                    .result
            });
        crate::PlatformRuntimeFactory::runtimeEventResponse(result)
    }
}

#[cfg(target_arch = "wasm32")]
impl OperitFlutterBridge {
    /// Preserves owner event ingress without blocking the browser event loop.
    pub(crate) async fn emitRuntimeEvent(&self, encoded: &str) -> String {
        let result = match crate::PlatformRuntimeFactory::runtimeEventRequest(encoded) {
            Ok(request) => self.prepareCall(request).execute().await.result,
            Err(error) => Err(error),
        };
        crate::PlatformRuntimeFactory::runtimeEventResponse(result)
    }

    /// Adapts the shared local Core API to a browser Promise.
    pub(crate) async fn call(&self, request: CoreCallRequest) -> CoreCallResponse {
        self.callShared(request).await
    }
    /// Executes the same validated push task inside the browser runtime.
    pub(crate) async fn pushItem(&self, item: CorePushItem) -> Result<(), CoreLinkError> {
        self.preparePushItem(item)?.execute().await
    }
    /// Executes the same push close task inside the browser runtime.
    pub(crate) async fn pushClose(&self, id: &str) -> Result<(), CoreLinkError> {
        self.preparePushClose(id)?.execute().await
    }
    /// Executes the same snapshot task inside the browser runtime.
    pub(crate) async fn watchSnapshot(
        &self,
        request: CoreWatchRequest,
    ) -> Result<operit_link::CoreEvent, CoreLinkError> {
        self.prepareWatchSnapshot(request).execute().await
    }
    /// Delivers the shared ordered watch stream to its explicit JavaScript callback.
    pub(crate) async fn watchStream(
        &self,
        id: String,
        request: CoreWatchRequest,
        callback: Function,
    ) -> Result<String, CoreLinkError> {
        let watch = self.prepareWatchStream(id.clone(), request)?.open().await?;
        wasm_bindgen_futures::spawn_local(async move {
            let result = watch
                .forward(move |id, event| {
                    let bytes = native_watch_event_vec(id, event);
                    callback
                        .call1(
                            &JsValue::NULL,
                            &js_sys::Uint8Array::from(bytes.as_slice()).into(),
                        )
                        .map(|_| ())
                        .map_err(|error| {
                            CoreLinkError::internal(format!(
                                "Browser watch callback failed: {error:?}"
                            ))
                        })
                })
                .await;
            if let Err(error) = result {
                operit_util::AppLogger::AppLogger::e("OperitFlutterBridge", &error.to_string());
            }
        });
        Ok(id)
    }
}
