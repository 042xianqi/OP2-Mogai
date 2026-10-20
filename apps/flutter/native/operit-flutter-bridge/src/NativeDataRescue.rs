//! Native offline ABI; all target selection remains behind platform_runtime.
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::sync::atomic::{AtomicU64, Ordering};
use operit_host_data_rescue::{NativeRescueSession, StorageLease, RescueProgress};
use serde_json::{json, Value};

static RUNTIME_LEASE: OnceLock<Mutex<Option<StorageLease>>> = OnceLock::new();
static RESCUE_SESSION: OnceLock<Mutex<Option<NativeRescueSession>>> = OnceLock::new();

static JOB: Mutex<Option<RescueJob>> = Mutex::new(None);
static NEXT_JOB: AtomicU64 = AtomicU64::new(1);

/// Retains one task result independently of the long-running session lock.
struct RescueJob {
    id: u64,
    progress: Arc<Mutex<Option<RescueProgress>>>,
    result: Arc<Mutex<Option<Result<Value, String>>>>,
}

/// Starts one background operation while keeping platform request queues available.
fn start_job(default_runtime: PathBuf, request: Value) -> Result<Value, String> {
    let operation = request["request"].clone();
    match operation["operation"].as_str() {
        Some("export" | "inspect" | "restore" | "resumeRestore" | "saveArchive" | "configureAutomatic") => (),
        _ => return Err("Unsupported background maintenance operation".into()),
    }
    let mut jobs = JOB.lock().map_err(|error| error.to_string())?;
    if jobs.is_some() { return Err("A maintenance job is already active".into()); }
    let session = RESCUE_SESSION.get_or_init(|| Mutex::new(None)).lock().map_err(|error| error.to_string())?;
    let progress = session.as_ref().ok_or("Open an exclusive maintenance session first")?.progress();
    *progress.lock().map_err(|error| error.to_string())? = None;
    drop(session);
    let id = NEXT_JOB.fetch_add(1, Ordering::Relaxed);
    let result = Arc::new(Mutex::new(None));
    let completion = result.clone();
    std::thread::Builder::new().name("data-rescue".into()).spawn(move || {
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| dispatch_session(default_runtime, operation)))
            .map_err(|_| "Maintenance worker panicked".to_string()).and_then(|value| value);
        *completion.lock().expect("Rescue job result lock poisoned") = Some(outcome);
    }).map_err(|error| error.to_string())?;
    *jobs = Some(RescueJob { id, progress, result });
    Ok(json!({"jobId": id}))
}

/// Reads the latest measured stage without waiting for filesystem work.
fn poll_job(request: &Value) -> Result<Value, String> {
    let mut jobs = JOB.lock().map_err(|error| error.to_string())?;
    let job = jobs.as_ref().ok_or("No maintenance job is active")?;
    if request["jobId"].as_u64() != Some(job.id) { return Err("Unknown maintenance job".into()); }
    let progress = job.progress.lock().map_err(|error| error.to_string())?.clone();
    let result = job.result.lock().map_err(|error| error.to_string())?.take();
    match result {
        None => Ok(json!({"done": false, "progress": progress})),
        Some(result) => {
            *jobs = None;
            match result {
                Ok(value) => Ok(json!({"done": true, "progress": progress, "value": value})),
                Err(error) => Err(error),
            }
        }
    }
}

/// Executes an offline maintenance command and preserves structured diagnostics at the ABI boundary.
pub fn execute(default_runtime: PathBuf, encoded: &str) -> String {
    let result = serde_json::from_str(encoded).map_err(|error| error.to_string())
        .and_then(|request| dispatch(default_runtime, request));
    match result {
        Ok(value) => json!({"ok": true, "value": value}).to_string(),
        Err(error) => json!({"ok": false, "error": error}).to_string(),
    }
}

/// Separates process access leases from exclusive rescue sessions before touching any business state.
fn dispatch(default_runtime: PathBuf, request: Value) -> Result<Value, String> {
    if request["operation"] == "launchMode" { return Ok(json!({"rescue": false})); }
    if request["operation"] == "runtimeLease" {
        let mut lease = RUNTIME_LEASE.get_or_init(|| Mutex::new(None)).lock().map_err(|error| error.to_string())?;
        if lease.is_none() { *lease = Some(StorageLease::acquire(&default_runtime, false)?); }
        return Ok(Value::Null);
    }
    if request["operation"] == "stopNormalProcess" {
        return Err("The current platform does not expose a process-stop operation through the rescue ABI".into());
    }
    if request["operation"] == "start" { return start_job(default_runtime, request); }
    if request["operation"] == "poll" { return poll_job(&request); }
    if JOB.lock().map_err(|error| error.to_string())?.is_some() { return Err("A maintenance job is already active".into()); }
    dispatch_session(default_runtime, request)
}

/// Executes one exclusive operation on the session owned by the background task.
fn dispatch_session(default_runtime: PathBuf, request: Value) -> Result<Value, String> {
    let mut session = RESCUE_SESSION.get_or_init(|| Mutex::new(None)).lock().map_err(|error| error.to_string())?;
    match request["operation"].as_str() {
        Some("open") => {
            if session.is_none() {
                let secrets = crate::PlatformRuntimeFactory::rescue_secret_store(&default_runtime)?;
                *session = Some(NativeRescueSession::open(default_runtime, secrets)?);
            }
            session.as_ref().ok_or("Maintenance session is unavailable")?.describe_startup()
        }
        Some("close") => { *session = None; Ok(Value::Null) }
        _ => session.as_mut().ok_or("Open an exclusive maintenance session first")?.execute(request),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reads progress while the session is locked and consumes terminal results exactly once.
    #[test]
    fn polling_is_independent_of_storage_and_preserves_task_failures() {
        let _session = RESCUE_SESSION.get_or_init(|| Mutex::new(None)).lock().unwrap();
        let progress = Arc::new(Mutex::new(Some(RescueProgress::new("restore", Some(1024), "bytes"))));
        let result = Arc::new(Mutex::new(None));
        *JOB.lock().unwrap() = Some(RescueJob { id: 42, progress: progress.clone(), result: result.clone() });
        assert!(poll_job(&json!({"jobId": 43})).is_err());
        let first = poll_job(&json!({"jobId": 42})).unwrap();
        assert_eq!(first["done"], false);
        assert_eq!(first["progress"]["completed"], 0);
        progress.lock().unwrap().as_mut().unwrap().completed = 512;
        assert_eq!(poll_job(&json!({"jobId": 42})).unwrap()["progress"]["completed"], 512);
        *result.lock().unwrap() = Some(Err("injected integrity failure".into()));
        assert_eq!(poll_job(&json!({"jobId": 42})).unwrap_err(), "injected integrity failure");
        assert!(JOB.lock().unwrap().is_none());
        assert!(poll_job(&json!({"jobId": 42})).is_err());
    }
}
