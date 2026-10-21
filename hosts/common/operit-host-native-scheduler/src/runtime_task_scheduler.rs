use operit_host_api::{
    HostError, HostResult, HostRuntimeAsyncTask, HostRuntimeTask, HostRuntimeTaskSchedulerHost,
    HostRuntimeTurnFuture,
};
use std::sync::{Arc, OnceLock};

const RUNTIME_WORKER_COUNT: usize = 16;
const BLOCKING_WORKER_COUNT: usize = 16;
const RUNTIME_WORKER_PREFIX: &str = "operit-runtime-worker";
const BLOCKING_WORKER_PREFIX: &str = "operit-runtime-blocking-worker";

/// Owns the process-wide coordination and blocking pools.
///
/// Every pool hands the next task to whichever worker is free. A worker stuck
/// in a blocking call therefore stops taking new tasks instead of accumulating
/// a private backlog that later stalls unrelated callers.
struct RuntimeTaskPool {
    coordination: tokio::sync::mpsc::UnboundedSender<HostRuntimeAsyncTask>,
    blocking: tokio::sync::mpsc::UnboundedSender<HostRuntimeAsyncTask>,
}

/// Creates one shared task queue consumed by the requested number of affine workers.
fn spawnTaskPool(
    prefix: &'static str,
    count: usize,
) -> Result<tokio::sync::mpsc::UnboundedSender<HostRuntimeAsyncTask>, String> {
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel::<HostRuntimeAsyncTask>();
    let receiver = Arc::new(tokio::sync::Mutex::new(receiver));
    for index in 0..count {
        let receiver = receiver.clone();
        let (ready, started) = std::sync::mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name(format!("{prefix}-{index}"))
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        let _ = ready.send(Err(error.to_string()));
                        return;
                    }
                };
                let _ = ready.send(Ok(()));
                // The runtime outlives every request and keeps its I/O and
                // spawned children alive without additional runtime threads.
                let local = tokio::task::LocalSet::new();
                local.block_on(&runtime, async {
                    loop {
                        // Only a free worker takes the next task. Holding the
                        // receive lock across the wait keeps one worker parked
                        // while every other worker remains available.
                        let task = receiver.lock().await.recv().await;
                        let Some(task) = task else { break };
                        // A factory panic is confined to its task, not the worker.
                        local.spawn_local(async move {
                            task().await;
                        });
                        // Let the LocalSet start the task and keep the receive
                        // handoff fair when many tasks arrive in one burst.
                        tokio::task::yield_now().await;
                    }
                });
            })
            .map_err(|e| e.to_string())?;
        started.recv().map_err(|e| e.to_string())??;
    }
    Ok(sender)
}

/// Initializes the process-wide coordination and blocking pools exactly once.
fn runtimeTaskPool() -> HostResult<&'static RuntimeTaskPool> {
    static POOL: OnceLock<Result<RuntimeTaskPool, String>> = OnceLock::new();
    POOL.get_or_init(|| {
        let coordination = spawnTaskPool(RUNTIME_WORKER_PREFIX, RUNTIME_WORKER_COUNT)?;
        let blocking = spawnTaskPool(BLOCKING_WORKER_PREFIX, BLOCKING_WORKER_COUNT)?;
        Ok(RuntimeTaskPool {
            coordination,
            blocking,
        })
    })
    .as_ref()
    .map_err(|e| HostError::new(format!("create runtime task pool failed: {e}")))
}

impl RuntimeTaskPool {
    /// Enqueues one coordination task on a free persistent worker.
    fn scheduleCoordination(&self, taskName: &str, task: HostRuntimeAsyncTask) -> HostResult<()> {
        self.coordination
            .send(task)
            .map_err(|_| HostError::new(format!("coordination task worker closed: {taskName}")))
    }

    /// Enqueues one possibly blocking task on a free blocking worker.
    fn scheduleBlocking(&self, taskName: &str, task: HostRuntimeAsyncTask) -> HostResult<()> {
        self.blocking
            .send(task)
            .map_err(|_| HostError::new(format!("blocking task worker closed: {taskName}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{rc::Rc, sync::mpsc, time::Duration};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// Waits for one pooled request to finish before accessing shared resources.
    fn runRequest(task: HostRuntimeAsyncTask) {
        let (done, completion) = mpsc::channel();
        NativeHostRuntimeTaskSchedulerHost
            .scheduleHostRuntimeAsyncTask(
                "test-request",
                Box::new(move || {
                    Box::pin(async move {
                        task().await;
                        done.send(()).unwrap();
                    })
                }),
            )
            .unwrap();
        completion.recv_timeout(Duration::from_secs(5)).unwrap();
    }

    /// Verifies socket I/O remains valid after the creating request ends.
    #[test]
    fn connectionSurvivesBetweenRequests() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (sender, receiver) = mpsc::channel();
        runRequest(Box::new(move || {
            Box::pin(async move {
                let stream = tokio::net::TcpStream::connect(address).await.unwrap();
                sender.send(stream).unwrap();
            })
        }));
        let mut stream = receiver.recv_timeout(Duration::from_secs(5)).unwrap();
        let (mut peer, _) = listener.accept().unwrap();
        runRequest(Box::new(move || {
            Box::pin(async move {
                // Start/finish pairing must be able to use the very same socket.
                std::io::Write::write_all(&mut peer, b"code").unwrap();
                let mut received = [0; 4];
                tokio::time::timeout(Duration::from_secs(5), stream.read_exact(&mut received))
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(&received, b"code");
                stream.write_all(b"done").await.unwrap();
            })
        }));
    }

    /// Verifies spawned receivers outlive their initiating request.
    #[test]
    fn spawnedReceiverSurvivesRequestCompletion() {
        let (release, released) = tokio::sync::oneshot::channel();
        let (done, completion) = mpsc::channel();
        runRequest(Box::new(move || {
            Box::pin(async move {
                tokio::spawn(async move {
                    released.await.unwrap();
                    tokio::time::sleep(Duration::from_millis(1)).await;
                    done.send(()).unwrap();
                });
            })
        }));
        release
            .send(())
            .expect("receiver was aborted when request ended");
        completion.recv_timeout(Duration::from_secs(5)).unwrap();
    }

    /// Verifies non-Send futures remain on the same pooled owner across waits.
    #[test]
    fn requestFutureCanRemainNonSend() {
        runRequest(Box::new(|| {
            Box::pin(async {
                let local = Rc::new(42);
                let thread = std::thread::current().id();
                tokio::time::sleep(Duration::from_millis(1)).await;
                assert_eq!(*local, 42);
                assert_eq!(std::thread::current().id(), thread);
            })
        }));
    }
}

/// Schedules native runtime tasks on process-wide coordination and blocking pools.
#[derive(Clone, Copy, Debug, Default)]
pub struct NativeHostRuntimeTaskSchedulerHost;

impl NativeHostRuntimeTaskSchedulerHost {
    /// Creates the native runtime task scheduler host.
    pub fn new() -> Self {
        Self
    }
}

impl HostRuntimeTaskSchedulerHost for NativeHostRuntimeTaskSchedulerHost {
    /// Reads the process-local monotonic clock used for native elapsed-time measurements.
    fn monotonicTimeMillis(&self) -> HostResult<u64> {
        static START: OnceLock<std::time::Instant> = OnceLock::new();
        Ok(START
            .get_or_init(std::time::Instant::now)
            .elapsed()
            .as_millis() as u64)
    }

    /// Executes synchronous work on the dedicated blocking pool.
    fn scheduleHostRuntimeTask(&self, taskName: &str, task: HostRuntimeTask) -> HostResult<()> {
        runtimeTaskPool()?
            .scheduleBlocking(taskName, Box::new(move || Box::pin(async move { task() })))
    }

    /// Creates and polls an asynchronous request on a free coordination worker.
    fn scheduleHostRuntimeAsyncTask(
        &self,
        taskName: &str,
        task: HostRuntimeAsyncTask,
    ) -> HostResult<()> {
        runtimeTaskPool()?.scheduleCoordination(taskName, task)
    }

    /// Creates and polls a possibly blocking request on a free blocking worker.
    fn scheduleHostRuntimeBlockingAsyncTask(
        &self,
        taskName: &str,
        task: HostRuntimeAsyncTask,
    ) -> HostResult<()> {
        runtimeTaskPool()?.scheduleBlocking(taskName, task)
    }

    /// Uses the coordination pool for cooperative callers without creating another executor.
    fn scheduleHostRuntimeCooperativeAsyncTask(
        &self,
        taskName: &str,
        task: HostRuntimeAsyncTask,
    ) -> HostResult<()> {
        self.scheduleHostRuntimeAsyncTask(taskName, task)
    }

    /// Waits asynchronously and executes delayed work on the coordination pool.
    fn scheduleDelayedHostRuntimeTask(
        &self,
        taskName: &str,
        delayMs: u64,
        task: HostRuntimeTask,
    ) -> HostResult<()> {
        self.scheduleHostRuntimeAsyncTask(
            taskName,
            Box::new(move || {
                Box::pin(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(delayMs)).await;
                    task();
                })
            }),
        )
    }

    /// Waits for a later native executor turn without occupying the caller thread.
    fn waitForHostRuntimeTaskTurn(&self) -> HostRuntimeTurnFuture {
        Box::pin(async {
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
            Ok(())
        })
    }

    /// Waits through the native timer executor for one platform-owned delay.
    fn waitForHostRuntimeDelay(&self, delayMs: u64) -> HostRuntimeTurnFuture {
        Box::pin(async move {
            tokio::time::sleep(std::time::Duration::from_millis(delayMs)).await;
            Ok(())
        })
    }
}

#[cfg(test)]
mod timer_clock_tests {
    use super::*;

    /// Verifies wall-clock timestamps are not used and readings advance with native delays.
    #[test]
    fn scheduler_clock_is_monotonic_and_shared_between_instances() {
        let scheduler = NativeHostRuntimeTaskSchedulerHost::new();
        let before = scheduler.monotonicTimeMillis().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let after = NativeHostRuntimeTaskSchedulerHost::new()
            .monotonicTimeMillis()
            .unwrap();
        assert!(after >= before + 1);
    }

    /// Verifies the local executor and its delegated timer use the same clock origin.
    #[test]
    fn local_scheduler_uses_the_native_timer_clock() {
        let local = crate::LocalHostRuntimeTaskSchedulerHost::new().unwrap();
        let before = NativeHostRuntimeTaskSchedulerHost
            .monotonicTimeMillis()
            .unwrap();
        let now = local.monotonicTimeMillis().unwrap();
        let after = NativeHostRuntimeTaskSchedulerHost
            .monotonicTimeMillis()
            .unwrap();
        assert!(before <= now && now <= after);
    }
}

#[cfg(test)]
mod cooperative_tests {
    use super::*;
    use std::{collections::HashSet, rc::Rc, sync::mpsc, time::Duration};

    /// Verifies cooperative tasks share the native pool and keep non-Send values affine.
    #[test]
    fn cooperative_tasks_reuse_threads_and_keep_non_send_futures_affine() {
        let scheduler = NativeHostRuntimeTaskSchedulerHost;
        let (send, receive) = mpsc::channel();
        for _ in 0..64 {
            let send = send.clone();
            scheduler
                .scheduleHostRuntimeCooperativeAsyncTask(
                    "test-cooperative",
                    Box::new(move || {
                        Box::pin(async move {
                            let local = Rc::new(7);
                            let thread = std::thread::current().id();
                            tokio::time::sleep(Duration::from_millis(1)).await;
                            assert_eq!(thread, std::thread::current().id());
                            assert_eq!(*local, 7);
                            send.send(thread).unwrap();
                        })
                    }),
                )
                .unwrap();
        }
        let threads = (0..64)
            .map(|_| receive.recv_timeout(Duration::from_secs(5)).unwrap())
            .collect::<HashSet<_>>();
        assert!(threads.len() <= RUNTIME_WORKER_COUNT);
    }

    /// Verifies suspended work leaves every owner available to another ready task.
    #[test]
    fn cooperative_waiting_task_does_not_block_other_tasks_on_the_same_worker() {
        let scheduler = NativeHostRuntimeTaskSchedulerHost;
        let (release, wait) = tokio::sync::oneshot::channel();
        let (done, receive) = mpsc::channel();
        scheduler
            .scheduleHostRuntimeCooperativeAsyncTask(
                "waiting",
                Box::new(move || {
                    Box::pin(async move {
                        wait.await.unwrap();
                    })
                }),
            )
            .unwrap();
        // Cover the whole pool so ready tasks also reach the waiting task's owner.
        for _ in 0..RUNTIME_WORKER_COUNT {
            let done = done.clone();
            scheduler
                .scheduleHostRuntimeCooperativeAsyncTask(
                    "ready",
                    Box::new(move || {
                        Box::pin(async move {
                            done.send(()).unwrap();
                        })
                    }),
                )
                .unwrap();
        }
        for _ in 0..RUNTIME_WORKER_COUNT {
            receive.recv_timeout(Duration::from_secs(5)).unwrap();
        }
        release.send(()).unwrap();
    }

    /// Verifies a panicking factory cannot destroy a worker or force a new thread.
    #[test]
    fn cooperative_factory_panic_does_not_kill_its_owner_thread() {
        let scheduler = NativeHostRuntimeTaskSchedulerHost;
        let (entered, receive) = mpsc::channel();
        scheduler
            .scheduleHostRuntimeCooperativeAsyncTask(
                "panicking-factory",
                Box::new(move || {
                    entered.send(std::thread::current().id()).unwrap();
                    panic!("test task factory panic");
                }),
            )
            .unwrap();
        let original = receive.recv_timeout(Duration::from_secs(5)).unwrap();
        let (done, receive) = mpsc::channel();
        // Enough consecutive submissions to cover all owners, even when other
        // scheduler tests concurrently advance the round-robin index.
        for _ in 0..64 {
            let done = done.clone();
            scheduler
                .scheduleHostRuntimeCooperativeAsyncTask(
                    "after-panic",
                    Box::new(move || {
                        Box::pin(async move {
                            done.send(std::thread::current().id()).unwrap();
                        })
                    }),
                )
                .unwrap();
        }
        let threads = (0..64)
            .map(|_| receive.recv_timeout(Duration::from_secs(5)).unwrap())
            .collect::<HashSet<_>>();
        assert!(threads.contains(&original));
        assert!(threads.len() <= RUNTIME_WORKER_COUNT);
    }

    /// Verifies spawned I/O children survive the completion of their parent request.
    #[test]
    fn cooperative_spawned_io_child_survives_parent_completion() {
        let scheduler = NativeHostRuntimeTaskSchedulerHost;
        let (release, wait) = tokio::sync::oneshot::channel();
        let (done, receive) = mpsc::channel();
        scheduler
            .scheduleHostRuntimeCooperativeAsyncTask(
                "parent",
                Box::new(move || {
                    Box::pin(async move {
                        tokio::spawn(async move {
                            wait.await.unwrap();
                            tokio::time::sleep(Duration::from_millis(1)).await;
                            done.send(()).unwrap();
                        });
                    })
                }),
            )
            .unwrap();
        release.send(()).unwrap();
        receive.recv_timeout(Duration::from_secs(5)).unwrap();
    }
}

#[cfg(test)]
mod pool_tests {
    use super::*;
    use std::{
        collections::HashSet,
        rc::Rc,
        sync::{mpsc, Mutex},
        time::{Duration, Instant},
    };

    /// Serializes tests that deliberately occupy the shared blocking pool.
    static BLOCKING_TEST_LOCK: Mutex<()> = Mutex::new(());

    /// Returns the worker name so tests can prove which pool ran a task.
    fn threadName() -> String {
        std::thread::current()
            .name()
            .unwrap_or_default()
            .to_string()
    }

    /// Verifies task kinds stay on their declared pool and the pools never overlap.
    #[test]
    fn task_kinds_use_disjoint_persistent_pools() {
        let _guard = BLOCKING_TEST_LOCK.lock().unwrap();
        let scheduler = NativeHostRuntimeTaskSchedulerHost::new();
        let (sender, receiver) = mpsc::channel();
        let mut first_workers = HashSet::new();
        for round in 0..2 {
            for index in 0..320 {
                let sender = sender.clone();
                let other = NativeHostRuntimeTaskSchedulerHost::new();
                match index % 5 {
                    0 => scheduler.scheduleHostRuntimeTask(
                        "pool-sync",
                        Box::new(move || {
                            sender.send(("blocking", threadName())).unwrap();
                        }),
                    ),
                    1 => other.scheduleHostRuntimeBlockingAsyncTask(
                        "pool-blocking-async",
                        Box::new(move || {
                            Box::pin(async move {
                                sender.send(("blocking", threadName())).unwrap();
                            })
                        }),
                    ),
                    2 => other.scheduleHostRuntimeAsyncTask(
                        "pool-async",
                        Box::new(move || {
                            Box::pin(async move {
                                let local = Rc::new(42);
                                let owner = std::thread::current().id();
                                tokio::time::sleep(Duration::from_millis(1)).await;
                                assert_eq!(owner, std::thread::current().id());
                                assert_eq!(*local, 42);
                                sender.send(("coordination", threadName())).unwrap();
                            })
                        }),
                    ),
                    3 => other.scheduleHostRuntimeCooperativeAsyncTask(
                        "pool-cooperative",
                        Box::new(move || {
                            Box::pin(async move {
                                sender.send(("coordination", threadName())).unwrap();
                            })
                        }),
                    ),
                    _ => scheduler.scheduleDelayedHostRuntimeTask(
                        "pool-delayed",
                        1,
                        Box::new(move || {
                            sender.send(("coordination", threadName())).unwrap();
                        }),
                    ),
                }
                .unwrap();
            }
            let workers = (0..320)
                .map(|_| receiver.recv_timeout(Duration::from_secs(10)).unwrap())
                .collect::<HashSet<_>>();
            let coordination = workers
                .iter()
                .filter(|(pool, _)| *pool == "coordination")
                .map(|(_, name)| name.clone())
                .collect::<HashSet<_>>();
            let blocking = workers
                .iter()
                .filter(|(pool, _)| *pool == "blocking")
                .map(|(_, name)| name.clone())
                .collect::<HashSet<_>>();
            assert_eq!(coordination.len(), RUNTIME_WORKER_COUNT);
            assert_eq!(blocking.len(), BLOCKING_WORKER_COUNT);
            assert!(coordination.is_disjoint(&blocking));
            assert!(coordination
                .iter()
                .all(|name| name.starts_with(RUNTIME_WORKER_PREFIX)));
            assert!(blocking
                .iter()
                .all(|name| name.starts_with(BLOCKING_WORKER_PREFIX)));
            if round == 0 {
                first_workers = workers;
            } else {
                assert_eq!(workers, first_workers);
            }
        }
    }

    /// Verifies a fully occupied blocking pool cannot delay coordination tasks.
    #[test]
    fn saturated_blocking_pool_does_not_delay_coordination_tasks() {
        let _guard = BLOCKING_TEST_LOCK.lock().unwrap();
        let scheduler = NativeHostRuntimeTaskSchedulerHost;
        let (entered, started) = mpsc::channel();
        for _ in 0..BLOCKING_WORKER_COUNT {
            let entered = entered.clone();
            scheduler
                .scheduleHostRuntimeBlockingAsyncTask(
                    "occupy-blocking",
                    Box::new(move || {
                        Box::pin(async move {
                            entered.send(()).unwrap();
                            std::thread::sleep(Duration::from_millis(300));
                        })
                    }),
                )
                .unwrap();
        }
        for _ in 0..BLOCKING_WORKER_COUNT {
            started.recv_timeout(Duration::from_secs(5)).unwrap();
        }
        let (done, finished) = mpsc::channel();
        let start = Instant::now();
        scheduler
            .scheduleHostRuntimeAsyncTask(
                "coordination-probe",
                Box::new(move || {
                    Box::pin(async move {
                        done.send(()).unwrap();
                    })
                }),
            )
            .unwrap();
        finished.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(
            start.elapsed() < Duration::from_millis(200),
            "coordination task waited behind blocked workers: {:?}",
            start.elapsed()
        );
    }

    /// Verifies more than sixteen pending streams suspend without blocking new requests.
    #[test]
    fn suspended_tasks_do_not_exhaust_the_worker_pool() {
        let scheduler = NativeHostRuntimeTaskSchedulerHost;
        let (entered, started) = mpsc::channel();
        let (completed, done) = mpsc::channel();
        let mut releases = Vec::new();
        for _ in 0..128 {
            let (release, wait) = tokio::sync::oneshot::channel();
            releases.push(release);
            let entered = entered.clone();
            let completed = completed.clone();
            scheduler
                .scheduleHostRuntimeAsyncTask(
                    "pool-waiting",
                    Box::new(move || {
                        Box::pin(async move {
                            entered.send(()).unwrap();
                            wait.await.unwrap();
                            completed.send(()).unwrap();
                        })
                    }),
                )
                .unwrap();
        }
        for _ in 0..128 {
            started.recv_timeout(Duration::from_secs(5)).unwrap();
        }
        let (sender, receiver) = mpsc::channel();
        for _ in 0..64 {
            let sender = sender.clone();
            scheduler
                .scheduleHostRuntimeAsyncTask(
                    "pool-ready",
                    Box::new(move || Box::pin(async move { sender.send(()).unwrap() })),
                )
                .unwrap();
        }
        for _ in 0..64 {
            receiver.recv_timeout(Duration::from_secs(5)).unwrap();
        }
        for release in releases {
            release.send(()).unwrap();
        }
        for _ in 0..128 {
            done.recv_timeout(Duration::from_secs(5)).unwrap();
        }
    }
}
