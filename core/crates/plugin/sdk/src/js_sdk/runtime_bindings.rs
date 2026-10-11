//! Canonical bindings from every JavaScript Tools method to the registered executor contract.

use super::tool_types::BuiltinToolName;

/// Binds one public Tools method to its registered executor and result presentation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JsToolBinding {
    /// Contains the dotted JavaScript namespace path.
    pub namespace: &'static str,
    /// Contains the public method name inside the namespace.
    pub method: &'static str,
    /// Selects the exact executor contract invoked through the common tool pipeline.
    pub target: JsToolTarget,
    /// Selects only how the public result is presented, without changing invocation transport.
    pub facade: JsToolFacade,
    /// Declares fixed executor parameters that are absent from the public SDK arguments.
    pub fixed_parameters: &'static [(&'static str, &'static str)],
}

/// Selects the executor identity in the single registration surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JsToolTarget {
    Builtin(BuiltinToolName),
    Runtime(RuntimeToolName),
}

/// Selects JavaScript presentation without declaring a second native transport.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JsToolFacade {
    Promise,
    AsyncIterator,
    Storage,
}

/// Identifies internal structured operations registered on the ordinary tool executor registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeToolName {
    StorageRequest,
    ChatSendMessage,
    ChatObserve,
    ChatCancel,
    ChatReadExtension,
    ChatWriteExtension,
    ChatDeleteExtension,
    ModelsListSummaries,
    TtsListConfigs,
    TtsGetCurrentConfigId,
    TtsSetCurrentConfigId,
    ThemeListConfigs,
    ThemeApplyConfig,
    CatalogReadToolSources,
}

impl RuntimeToolName {
    /// Returns the canonical registry name using the same enum spelling consumed by code generation.
    pub fn registry_name(self) -> String {
        serde_json::to_value(self).expect("Runtime operation identity must serialize").as_str()
            .expect("Runtime operation identity must be text").to_owned()
    }
}

/// Declares parameter conventions shared by every generated built-in tool binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JsToolParameterRules {
    /// Lists arguments whose object fields become executor parameters.
    pub flatten_arguments: &'static [&'static str],
    /// Lists union argument suffixes whose scalar branch uses the name before the suffix.
    pub scalar_union_suffixes: &'static [&'static str],
    /// Lists namespaces whose flattened object fields retain their public spelling.
    pub preserve_object_namespaces: &'static [&'static str],
    /// Maps exact namespace, method and positional argument names to executor field names.
    pub argument_names: &'static [(&'static str, &'static str, &'static str, &'static str)],
}

/// Contains the sole declaration of executor argument naming and flattening conventions.
pub const JS_TOOL_PARAMETER_RULES: JsToolParameterRules = JsToolParameterRules {
    flatten_arguments: &["options", "params", "updates"],
    scalar_union_suffixes: &["OrOptions", "OrParams"],
    preserve_object_namespaces: &["Net"],
    argument_names: &[
        ("Files", "writeBinary", "base64Content", "base64Content"),
        ("Files", "apply", "newContent", "new"),
        ("Files", "create", "newContent", "new"),
        ("Files", "edit", "newContent", "new"),
        ("Files", "edit", "oldContent", "old"),
        ("System", "sleep", "milliseconds", "duration_ms"),
        ("System", "listApps", "includeSystem", "include_system_apps"),
    ],
};

/// Declares one versioned JavaScript Tools method implementation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JsToolApiVariant {
    /// Contains the JavaScript namespace containing the gated method.
    pub namespace: &'static str,
    /// Contains the public method name controlled by the gate.
    pub method: &'static str,
    /// Contains the first version selecting this implementation within its API family.
    pub since: &'static str,
    /// Contains an exclusive upper bound; None ends at the next major API family.
    pub until: Option<&'static str>,
    /// Contains an optional variant-specific JavaScript parameter list.
    pub arguments: Option<&'static [&'static str]>,
}

/// Contains every active JavaScript Tools method binding.
#[rustfmt::skip]
pub const JS_TOOL_BINDINGS: &[JsToolBinding] = &[
    JsToolBinding { namespace: "Edge", method: "execute", target: JsToolTarget::Builtin(BuiltinToolName::EdgeExecute), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Io", method: "execute", target: JsToolTarget::Builtin(BuiltinToolName::IoExecute), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Chat", method: "agentStatus", target: JsToolTarget::Builtin(BuiltinToolName::AgentStatus), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Chat", method: "createNew", target: JsToolTarget::Builtin(BuiltinToolName::CreateNewChat), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Chat", method: "deleteChat", target: JsToolTarget::Builtin(BuiltinToolName::DeleteChat), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Chat", method: "findChat", target: JsToolTarget::Builtin(BuiltinToolName::FindChat), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Chat", method: "getMessages", target: JsToolTarget::Builtin(BuiltinToolName::GetChatMessages), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Chat", method: "getMessagesRange", target: JsToolTarget::Builtin(BuiltinToolName::GetChatMessagesRange), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Chat", method: "call", target: JsToolTarget::Builtin(BuiltinToolName::CallChatModel), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Chat", method: "listAll", target: JsToolTarget::Builtin(BuiltinToolName::ListChats), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Chat", method: "listChats", target: JsToolTarget::Builtin(BuiltinToolName::ListChats), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Chat", method: "startService", target: JsToolTarget::Builtin(BuiltinToolName::StartChatService), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Chat", method: "stopService", target: JsToolTarget::Builtin(BuiltinToolName::StopChatService), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Chat", method: "switchTo", target: JsToolTarget::Builtin(BuiltinToolName::SwitchChat), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Chat", method: "updateTitle", target: JsToolTarget::Builtin(BuiltinToolName::UpdateChatTitle), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Chat", method: "updateGroup", target: JsToolTarget::Builtin(BuiltinToolName::UpdateChatGroup), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Chat", method: "updatePinned", target: JsToolTarget::Builtin(BuiltinToolName::UpdateChatPinned), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Chat", method: "updateLocked", target: JsToolTarget::Builtin(BuiltinToolName::UpdateChatLocked), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Chat", method: "reorder", target: JsToolTarget::Builtin(BuiltinToolName::ReorderChats), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Files", method: "apply", target: JsToolTarget::Builtin(BuiltinToolName::ApplyFile), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Files", method: "copy", target: JsToolTarget::Builtin(BuiltinToolName::CopyFile), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Files", method: "create", target: JsToolTarget::Builtin(BuiltinToolName::CreateFile), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Files", method: "deleteFile", target: JsToolTarget::Builtin(BuiltinToolName::DeleteFile), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Files", method: "download", target: JsToolTarget::Builtin(BuiltinToolName::DownloadFile), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Files", method: "edit", target: JsToolTarget::Builtin(BuiltinToolName::EditFile), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Files", method: "exists", target: JsToolTarget::Builtin(BuiltinToolName::FileExists), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Files", method: "find", target: JsToolTarget::Builtin(BuiltinToolName::FindFiles), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Files", method: "grep", target: JsToolTarget::Builtin(BuiltinToolName::GrepCode), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Files", method: "grepContext", target: JsToolTarget::Builtin(BuiltinToolName::GrepContext), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Files", method: "info", target: JsToolTarget::Builtin(BuiltinToolName::FileInfo), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Files", method: "list", target: JsToolTarget::Builtin(BuiltinToolName::ListFiles), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Files", method: "mkdir", target: JsToolTarget::Builtin(BuiltinToolName::MakeDirectory), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Files", method: "move", target: JsToolTarget::Builtin(BuiltinToolName::MoveFile), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Files", method: "open", target: JsToolTarget::Builtin(BuiltinToolName::OpenFile), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Files", method: "read", target: JsToolTarget::Builtin(BuiltinToolName::ReadFileFull), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Files", method: "readBinary", target: JsToolTarget::Builtin(BuiltinToolName::ReadFileBinary), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Files", method: "readPart", target: JsToolTarget::Builtin(BuiltinToolName::ReadFilePart), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Files", method: "share", target: JsToolTarget::Builtin(BuiltinToolName::ShareFile), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Files", method: "unzip", target: JsToolTarget::Builtin(BuiltinToolName::UnzipFiles), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Files", method: "write", target: JsToolTarget::Builtin(BuiltinToolName::WriteFile), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Files", method: "writeBinary", target: JsToolTarget::Builtin(BuiltinToolName::WriteFileBinary), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Files", method: "zip", target: JsToolTarget::Builtin(BuiltinToolName::ZipFiles), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Net", method: "browserClick", target: JsToolTarget::Builtin(BuiltinToolName::BrowserClick), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Net", method: "browserClose", target: JsToolTarget::Builtin(BuiltinToolName::BrowserClose), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Net", method: "browserCloseAll", target: JsToolTarget::Builtin(BuiltinToolName::BrowserCloseAll), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Net", method: "browserConsoleMessages", target: JsToolTarget::Builtin(BuiltinToolName::BrowserConsoleMessages), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Net", method: "browserDrag", target: JsToolTarget::Builtin(BuiltinToolName::BrowserDrag), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Net", method: "browserEvaluate", target: JsToolTarget::Builtin(BuiltinToolName::BrowserEvaluate), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Net", method: "browserFileUpload", target: JsToolTarget::Builtin(BuiltinToolName::BrowserFileUpload), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Net", method: "browserFillForm", target: JsToolTarget::Builtin(BuiltinToolName::BrowserFillForm), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Net", method: "browserHandleDialog", target: JsToolTarget::Builtin(BuiltinToolName::BrowserHandleDialog), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Net", method: "browserHover", target: JsToolTarget::Builtin(BuiltinToolName::BrowserHover), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Net", method: "browserNavigate", target: JsToolTarget::Builtin(BuiltinToolName::BrowserNavigate), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Net", method: "browserNavigateBack", target: JsToolTarget::Builtin(BuiltinToolName::BrowserNavigateBack), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Net", method: "browserNetworkRequests", target: JsToolTarget::Builtin(BuiltinToolName::BrowserNetworkRequests), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Net", method: "browserPressKey", target: JsToolTarget::Builtin(BuiltinToolName::BrowserPressKey), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Net", method: "browserResize", target: JsToolTarget::Builtin(BuiltinToolName::BrowserResize), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Net", method: "browserRunCode", target: JsToolTarget::Builtin(BuiltinToolName::BrowserRunCode), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Net", method: "browserSelectOption", target: JsToolTarget::Builtin(BuiltinToolName::BrowserSelectOption), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Net", method: "browserSnapshot", target: JsToolTarget::Builtin(BuiltinToolName::BrowserSnapshot), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Net", method: "browserTabs", target: JsToolTarget::Builtin(BuiltinToolName::BrowserTabs), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Net", method: "browserTakeScreenshot", target: JsToolTarget::Builtin(BuiltinToolName::BrowserTakeScreenshot), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Net", method: "browserType", target: JsToolTarget::Builtin(BuiltinToolName::BrowserType), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Net", method: "browserWaitFor", target: JsToolTarget::Builtin(BuiltinToolName::BrowserWaitFor), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Net", method: "http", target: JsToolTarget::Builtin(BuiltinToolName::HttpRequest), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Net", method: "httpGet", target: JsToolTarget::Builtin(BuiltinToolName::HttpRequest), facade: JsToolFacade::Promise, fixed_parameters: &[("method", "GET")] },
    JsToolBinding { namespace: "Net", method: "httpPost", target: JsToolTarget::Builtin(BuiltinToolName::HttpRequest), facade: JsToolFacade::Promise, fixed_parameters: &[("method", "POST")] },
    JsToolBinding { namespace: "Net", method: "uploadFile", target: JsToolTarget::Builtin(BuiltinToolName::MultipartRequest), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Net", method: "visit", target: JsToolTarget::Builtin(BuiltinToolName::VisitWeb), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Net.cookies", method: "clear", target: JsToolTarget::Builtin(BuiltinToolName::ManageCookies), facade: JsToolFacade::Promise, fixed_parameters: &[("action", "clear")] },
    JsToolBinding { namespace: "Net.cookies", method: "get", target: JsToolTarget::Builtin(BuiltinToolName::ManageCookies), facade: JsToolFacade::Promise, fixed_parameters: &[("action", "get")] },
    JsToolBinding { namespace: "Net.cookies", method: "set", target: JsToolTarget::Builtin(BuiltinToolName::ManageCookies), facade: JsToolFacade::Promise, fixed_parameters: &[("action", "set")] },
    JsToolBinding { namespace: "SoftwareSettings", method: "exec", target: JsToolTarget::Builtin(BuiltinToolName::ExecuteCliCommand), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "SoftwareSettings", method: "readEnvironmentVariable", target: JsToolTarget::Builtin(BuiltinToolName::ReadEnvironmentVariable), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "SoftwareSettings", method: "writeEnvironmentVariable", target: JsToolTarget::Builtin(BuiltinToolName::WriteEnvironmentVariable), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System", method: "getAppUsageTime", target: JsToolTarget::Builtin(BuiltinToolName::GetAppUsageTime), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System", method: "getDeviceInfo", target: JsToolTarget::Builtin(BuiltinToolName::DeviceInfo), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System", method: "getLocation", target: JsToolTarget::Builtin(BuiltinToolName::GetDeviceLocation), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System", method: "getNotifications", target: JsToolTarget::Builtin(BuiltinToolName::GetNotifications), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System", method: "getSetting", target: JsToolTarget::Builtin(BuiltinToolName::GetSystemSetting), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System", method: "installApp", target: JsToolTarget::Builtin(BuiltinToolName::InstallApp), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System", method: "listApps", target: JsToolTarget::Builtin(BuiltinToolName::ListInstalledApps), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System", method: "sendNotification", target: JsToolTarget::Builtin(BuiltinToolName::SendNotification), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System", method: "setSetting", target: JsToolTarget::Builtin(BuiltinToolName::ModifySystemSetting), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System", method: "sleep", target: JsToolTarget::Builtin(BuiltinToolName::Sleep), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System", method: "startApp", target: JsToolTarget::Builtin(BuiltinToolName::StartApp), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System", method: "stopApp", target: JsToolTarget::Builtin(BuiltinToolName::StopApp), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System", method: "toast", target: JsToolTarget::Builtin(BuiltinToolName::Toast), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System", method: "uninstallApp", target: JsToolTarget::Builtin(BuiltinToolName::UninstallApp), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System", method: "usePackage", target: JsToolTarget::Builtin(BuiltinToolName::UsePackage), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System.bluetooth", method: "accept", target: JsToolTarget::Builtin(BuiltinToolName::BluetoothAccept), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System.bluetooth", method: "close", target: JsToolTarget::Builtin(BuiltinToolName::BluetoothClose), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System.bluetooth", method: "connect", target: JsToolTarget::Builtin(BuiltinToolName::BluetoothConnect), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System.bluetooth", method: "getState", target: JsToolTarget::Builtin(BuiltinToolName::GetBluetoothState), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System.bluetooth", method: "listBondedDevices", target: JsToolTarget::Builtin(BuiltinToolName::ListBluetoothBondedDevices), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System.bluetooth", method: "listen", target: JsToolTarget::Builtin(BuiltinToolName::BluetoothListen), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System.bluetooth", method: "read", target: JsToolTarget::Builtin(BuiltinToolName::BluetoothRead), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System.bluetooth", method: "requestEnable", target: JsToolTarget::Builtin(BuiltinToolName::RequestEnableBluetooth), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System.bluetooth", method: "requestPermission", target: JsToolTarget::Builtin(BuiltinToolName::RequestBluetoothPermission), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System.bluetooth", method: "scan", target: JsToolTarget::Builtin(BuiltinToolName::ScanBluetoothDevices), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System.bluetooth", method: "send", target: JsToolTarget::Builtin(BuiltinToolName::BluetoothSend), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System.bluetooth", method: "sendAndRead", target: JsToolTarget::Builtin(BuiltinToolName::BluetoothSendAndRead), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System.bluetooth.ble", method: "connect", target: JsToolTarget::Builtin(BuiltinToolName::BluetoothBleConnect), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System.bluetooth.ble", method: "discoverServices", target: JsToolTarget::Builtin(BuiltinToolName::BluetoothBleDiscoverServices), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System.bluetooth.ble", method: "readCharacteristic", target: JsToolTarget::Builtin(BuiltinToolName::BluetoothBleReadCharacteristic), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System.bluetooth.ble", method: "readNotifications", target: JsToolTarget::Builtin(BuiltinToolName::BluetoothBleReadNotifications), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System.bluetooth.ble", method: "subscribe", target: JsToolTarget::Builtin(BuiltinToolName::BluetoothBleSubscribeCharacteristic), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System.bluetooth.ble", method: "writeAndReadCharacteristic", target: JsToolTarget::Builtin(BuiltinToolName::BluetoothBleWriteAndReadCharacteristic), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System.bluetooth.ble", method: "writeCharacteristic", target: JsToolTarget::Builtin(BuiltinToolName::BluetoothBleWriteCharacteristic), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System.music", method: "pause", target: JsToolTarget::Builtin(BuiltinToolName::MusicPause), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System.music", method: "play", target: JsToolTarget::Builtin(BuiltinToolName::MusicPlay), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System.music", method: "resume", target: JsToolTarget::Builtin(BuiltinToolName::MusicResume), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System.music", method: "seek", target: JsToolTarget::Builtin(BuiltinToolName::MusicSeek), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System.music", method: "setVolume", target: JsToolTarget::Builtin(BuiltinToolName::MusicSetVolume), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System.music", method: "status", target: JsToolTarget::Builtin(BuiltinToolName::MusicStatus), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System.music", method: "stop", target: JsToolTarget::Builtin(BuiltinToolName::MusicStop), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System.terminal", method: "close", target: JsToolTarget::Builtin(BuiltinToolName::CloseTerminalSession), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System.terminal", method: "create", target: JsToolTarget::Builtin(BuiltinToolName::CreateTerminalSession), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System.terminal", method: "exec", target: JsToolTarget::Builtin(BuiltinToolName::ExecuteInTerminalSession), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System.terminal", method: "execStreaming", target: JsToolTarget::Builtin(BuiltinToolName::ExecuteInTerminalSessionStreaming), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System.terminal", method: "hiddenExec", target: JsToolTarget::Builtin(BuiltinToolName::ExecuteHiddenTerminalCommand), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System.terminal", method: "info", target: JsToolTarget::Builtin(BuiltinToolName::GetTerminalInfo), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System.terminal", method: "input", target: JsToolTarget::Builtin(BuiltinToolName::InputInTerminalSession), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "System.terminal", method: "screen", target: JsToolTarget::Builtin(BuiltinToolName::GetTerminalSessionScreen), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Storage", method: "request", target: JsToolTarget::Runtime(RuntimeToolName::StorageRequest), facade: JsToolFacade::Storage, fixed_parameters: &[] },
    JsToolBinding { namespace: "Chat", method: "sendMessage", target: JsToolTarget::Runtime(RuntimeToolName::ChatSendMessage), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Chat", method: "sendMessageStreaming", target: JsToolTarget::Runtime(RuntimeToolName::ChatObserve), facade: JsToolFacade::AsyncIterator, fixed_parameters: &[] },
    JsToolBinding { namespace: "Chat", method: "cancel", target: JsToolTarget::Runtime(RuntimeToolName::ChatCancel), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Chat", method: "readExtension", target: JsToolTarget::Runtime(RuntimeToolName::ChatReadExtension), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Chat", method: "writeExtension", target: JsToolTarget::Runtime(RuntimeToolName::ChatWriteExtension), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Chat", method: "deleteExtension", target: JsToolTarget::Runtime(RuntimeToolName::ChatDeleteExtension), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Models", method: "listSummaries", target: JsToolTarget::Runtime(RuntimeToolName::ModelsListSummaries), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Tts", method: "listConfigs", target: JsToolTarget::Runtime(RuntimeToolName::TtsListConfigs), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Tts", method: "getCurrentConfigId", target: JsToolTarget::Runtime(RuntimeToolName::TtsGetCurrentConfigId), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Tts", method: "setCurrentConfigId", target: JsToolTarget::Runtime(RuntimeToolName::TtsSetCurrentConfigId), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Theme", method: "listConfigs", target: JsToolTarget::Runtime(RuntimeToolName::ThemeListConfigs), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Theme", method: "applyConfig", target: JsToolTarget::Runtime(RuntimeToolName::ThemeApplyConfig), facade: JsToolFacade::Promise, fixed_parameters: &[] },
    JsToolBinding { namespace: "Catalog", method: "readToolSources", target: JsToolTarget::Runtime(RuntimeToolName::CatalogReadToolSources), facade: JsToolFacade::Promise, fixed_parameters: &[] },
];

/// Contains every versioned implementation applied to generated Tools methods.
pub const JS_TOOL_API_VARIANTS: &[JsToolApiVariant] = &[
    JsToolApiVariant {
        namespace: "Chat",
        method: "call",
        since: "1.0.1",
        until: Some("2.0.0"),
        arguments: None,
    },
    JsToolApiVariant {
        namespace: "Chat",
        method: "call",
        since: "2.0.0",
        until: None,
        arguments: None,
    },
];

