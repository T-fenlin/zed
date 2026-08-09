use crate::{PlatformDispatcher, RunnableMeta};
use async_task::Runnable;
use chrono::{DateTime, Utc};
use futures::channel::oneshot;
use scheduler::Instant;
use scheduler::{
    Clock, LocalExecutor, Priority, Scheduler, SessionId, Task, TestScheduler, Timer,
    spawn_dedicated_thread,
};
#[cfg(not(target_family = "wasm"))]
use std::task::{Context, Poll};
use std::{
    any::Any,
    future::Future,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicU16, Ordering},
    },
    time::Duration,
};

/// A production implementation of [`Scheduler`] that wraps a [`PlatformDispatcher`].
///
/// This allows GPUI to use the scheduler crate's executor types with the platform's
/// native dispatch mechanisms (e.g., Grand Central Dispatch on macOS).
pub struct PlatformScheduler {
    dispatcher: Arc<dyn PlatformDispatcher>,
    clock: Arc<PlatformClock>,
    next_session_id: AtomicU16,
}

impl PlatformScheduler {
    pub fn new(dispatcher: Arc<dyn PlatformDispatcher>) -> Self {
        Self {
            dispatcher: dispatcher.clone(),
            clock: Arc::new(PlatformClock { dispatcher }),
            next_session_id: AtomicU16::new(0),
        }
    }

    pub fn foreground_executor(self: &Arc<Self>) -> LocalExecutor {
        let session_id = self.next_session_id();
        let scheduler = Arc::downgrade(self);
        LocalExecutor::new(session_id, self.clone(), move |runnable| {
            if let Some(scheduler) = scheduler.upgrade() {
                scheduler.schedule_local(session_id, runnable);
            }
        })
    }

    fn next_session_id(&self) -> SessionId {
        SessionId::new(self.next_session_id.fetch_add(1, Ordering::SeqCst))
    }
}

impl Scheduler for PlatformScheduler {
    fn block(
        &self,
        _session_id: Option<SessionId>,
        #[cfg_attr(target_family = "wasm", allow(unused_mut))] mut future: Pin<
            &mut dyn Future<Output = ()>,
        >,
        #[cfg_attr(target_family = "wasm", allow(unused_variables))] timeout: Option<Duration>,
    ) -> bool {
        #[cfg(target_family = "wasm")]
        {
            let _ = (&future, &timeout);
            panic!("无法在 wasm 上阻塞")
        }
        #[cfg(not(target_family = "wasm"))]
        {
            use waker_fn::waker_fn;
            let deadline = timeout.map(|t| Instant::now() + t);
            let parker = parking::Parker::new();
            let unparker = parker.unparker();
            let waker = waker_fn(move || {
                unparker.unpark();
            });
            let mut cx = Context::from_waker(&waker);
            if let Poll::Ready(()) = future.as_mut().poll(&mut cx) {
                return true;
            }

            let park_deadline = |deadline: Instant| {
                // Timer expirations are only delivered every ~15.6 milliseconds by default on Windows.
                // We increase the resolution during this wait so that short timeouts stay reasonably short.
                let _timer_guard = self.dispatcher.increase_timer_resolution();
                parker.park_deadline(deadline)
            };

            loop {
                match deadline {
                    Some(deadline) if !park_deadline(deadline) && deadline <= Instant::now() => {
                        return false;
                    }
                    Some(_) => (),
                    None => parker.park(),
                }
                if let Poll::Ready(()) = future.as_mut().poll(&mut cx) {
                    break true;
                }
            }
        }
    }

    fn schedule_local(&self, _session_id: SessionId, runnable: Runnable<RunnableMeta>) {
        self.dispatcher
            .dispatch_on_main_thread(runnable, Priority::default());
    }

    fn schedule_background_with_priority(
        &self,
        runnable: Runnable<RunnableMeta>,
        priority: Priority,
    ) {
        self.dispatcher.dispatch(runnable, priority);
    }

    fn spawn_realtime(&self, f: Box<dyn FnOnce() + Send>) {
        self.dispatcher.spawn_realtime(f);
    }

    #[track_caller]
    fn timer(&self, duration: Duration) -> Timer {
        let (tx, rx) = oneshot::channel();
        let dispatcher = self.dispatcher.clone();

        // Create a runnable that will send the completion signal
        let location = std::panic::Location::caller();
        let (runnable, _task) = async_task::Builder::new()
            .metadata(RunnableMeta {
                location,
                spawned: scheduler::SpawnTime(Instant::now()),
            })
            .spawn(
                move |_| async move {
                    let _ = tx.send(());
                },
                move |runnable| {
                    dispatcher.dispatch_after(duration, runnable);
                },
            );
        runnable.schedule();

        Timer::new(rx)
    }

    fn clock(&self) -> Arc<dyn Clock> {
        self.clock.clone()
    }

    fn spawn_dedicated(
        self: Arc<Self>,
        f: Box<
            dyn FnOnce(
                    LocalExecutor,
                )
                    -> Pin<Box<dyn Future<Output = Box<dyn Any + Send + Sync>> + 'static>>
                + Send
                + 'static,
        >,
    ) -> Task<Box<dyn Any + Send + Sync>> {
        let session_id = self.next_session_id();
        spawn_dedicated_thread(session_id, self, move |executor| f(executor))
    }

    fn as_test(&self) -> Option<&TestScheduler> {
        None
    }
}

/// A production clock that uses the platform dispatcher's time.
struct PlatformClock {
    dispatcher: Arc<dyn PlatformDispatcher>,
}

impl Clock for PlatformClock {
    fn utc_now(&self) -> DateTime<Utc> {
        Utc::now()
    }

    fn now(&self) -> Instant {
        self.dispatcher.now()
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;
    use crate::RunnableVariant;
    use scheduler::BackgroundExecutor;
    use std::time::Instant as StdInstant;

    // `spawn_dedicated` shouldn't touch the platform dispatcher at all;
    // panicking on every method ensures the test catches it if it does.
    struct SmokeDispatcher;

    impl PlatformDispatcher for SmokeDispatcher {
        fn is_main_thread(&self) -> bool {
            false
        }
        fn dispatch(&self, _runnable: RunnableVariant, _priority: Priority) {
            panic!("在此测试中不应要求 SmokeDispatcher 进行调度");
        }
        fn dispatch_on_main_thread(&self, _runnable: RunnableVariant, _priority: Priority) {
            panic!("SmokeDispatcher 未实现主线程");
        }
        fn dispatch_after(&self, _duration: Duration, _runnable: RunnableVariant) {
            panic!("SmokeDispatcher 未实现定时器");
        }
        fn spawn_realtime(&self, _f: Box<dyn FnOnce() + Send>) {
            panic!("SmokeDispatcher 未实现实时调度");
        }
    }

    #[test]
    fn spawn_dedicated_runs_on_a_real_separate_thread() {
        let background =
            BackgroundExecutor::new(Arc::new(PlatformScheduler::new(Arc::new(SmokeDispatcher))));
        let started = StdInstant::now();
        let task = background.spawn_dedicated(|_executor| async move {
            // A genuine blocking syscall on the dedicated thread. If
            // `spawn_dedicated` were running the future on any shared
            // executor, this would stall that executor.
            let thread_id_before = std::thread::current().id();
            std::thread::sleep(Duration::from_millis(50));
            let thread_id_after = std::thread::current().id();
            assert_eq!(thread_id_before, thread_id_after);
            (thread_id_before, "slept")
        });
        let (dedicated_thread_id, message) = futures::executor::block_on(task);
        let elapsed = started.elapsed();
        assert_eq!(message, "slept");
        assert_ne!(
            dedicated_thread_id,
            std::thread::current().id(),
            "专用 future 在测试线程上运行"
        );
        assert!(
            elapsed >= Duration::from_millis(40),
            "预期专用线程真正休眠,已用时间 = {:?}",
            elapsed
        );
    }

    #[test]
    fn spawn_dedicated_returns_not_send_future_output() {
        // The whole point of `spawn_dedicated` is that the future can be
        // `!Send`. Constructing one with `Rc<RefCell<_>>` ensures the
        // signature actually permits it.
        use std::cell::RefCell;
        use std::rc::Rc;

        let background =
            BackgroundExecutor::new(Arc::new(PlatformScheduler::new(Arc::new(SmokeDispatcher))));
        let task = background.spawn_dedicated(|_executor| async move {
            let state = Rc::new(RefCell::new(0_i32));
            for _ in 0..3 {
                *state.borrow_mut() += 1;
            }
            *state.borrow()
        });
        let output = futures::executor::block_on(task);
        assert_eq!(output, 3);
    }

    #[test]
    fn spawn_dedicated_dropping_task_cancels_future() {
        use parking_lot::Mutex;
        use std::sync::mpsc;

        let background =
            BackgroundExecutor::new(Arc::new(PlatformScheduler::new(Arc::new(SmokeDispatcher))));

        let (started_tx, started_rx) = mpsc::channel::<()>();
        let (after_park_tx, after_park_rx) = mpsc::channel::<()>();
        let observed_post_await_write = Arc::new(Mutex::new(false));

        let task = {
            let observed_post_await_write = observed_post_await_write.clone();
            background.spawn_dedicated(move |_executor| async move {
                // Announce that the future is live on the dedicated thread.
                started_tx
                    .send(())
                    .expect("必须收到启动信号");
                // Park forever. Dropping the `Task` must cancel us here so
                // the code below this `await` never runs.
                futures::future::pending::<()>().await;
                *observed_post_await_write.lock() = true;
                after_park_tx
                    .send(())
                    .expect("必须收到驻留后信号");
            })
        };

        // Wait until the dedicated future is actually parked at the await.
        started_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("专用 future 启动失败");

        // Drop the root Task: this must cancel the future.
        drop(task);

        // If cancellation works, the future never advances past `pending`,
        // so this recv must time out.
        assert!(
            after_park_rx
                .recv_timeout(Duration::from_millis(100))
                .is_err(),
            "专用 future 在其 Task 被丢弃后越过了 await"
        );
        assert!(
            !*observed_post_await_write.lock(),
            "专用 future 在取消点之后运行了代码"
        );
    }

    #[test]
    fn spawn_dedicated_thread_tears_down_after_work_completes() {
        use std::sync::mpsc;

        // Fires from `Drop` so we observe teardown of the dedicated future's
        // captured state on whichever thread runs its destructor.
        struct DropSignal {
            tx: Option<mpsc::Sender<std::thread::ThreadId>>,
        }
        impl Drop for DropSignal {
            fn drop(&mut self) {
                if let Some(tx) = self.tx.take() {
                    let _ = tx.send(std::thread::current().id());
                }
            }
        }

        let background =
            BackgroundExecutor::new(Arc::new(PlatformScheduler::new(Arc::new(SmokeDispatcher))));
        let (started_tx, started_rx) = mpsc::channel::<std::thread::ThreadId>();
        let (drop_tx, drop_rx) = mpsc::channel::<std::thread::ThreadId>();

        let task = background.spawn_dedicated(move |_executor| async move {
            // Captured by the future's state. When the future completes and
            // its state is dropped on the dedicated thread, this guard's
            // `Drop` fires and reports the thread id it ran on.
            let _guard = DropSignal { tx: Some(drop_tx) };
            started_tx
                .send(std::thread::current().id())
                .expect("必须收到启动信号");
            // Future returns immediately. The dedicated thread should then
            // drop the future (firing _guard), exit the recv loop, and exit.
        });

        let dedicated_thread_id = started_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("专用 future 启动失败");
        assert_ne!(
            dedicated_thread_id,
            std::thread::current().id(),
            "专用 future 在测试线程上运行"
        );

        // Drive the root task to completion so its body finishes.
        futures::executor::block_on(task);

        // The guard's drop runs from the dedicated thread as it tears down
        // the future's captured state. If the executor/recv-loop were
        // keeping the future alive past task completion, this would hang.
        let drop_thread_id = drop_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("任务完成后,专用 future 的捕获状态未被丢弃");
        assert_eq!(
            drop_thread_id, dedicated_thread_id,
            "专用 future 的捕获状态必须在专用线程上丢弃,而不是在其他地方"
        );
    }

    #[test]
    fn spawn_dedicated_detached_child_outlives_root() {
        use std::sync::mpsc;

        let background =
            BackgroundExecutor::new(Arc::new(PlatformScheduler::new(Arc::new(SmokeDispatcher))));

        // `gate_rx` lets the detached child park until the test explicitly
        // releases it — after we've already observed the root completing.
        let (gate_tx, gate_rx) = mpsc::channel::<()>();
        let (child_done_tx, child_done_rx) = mpsc::channel::<std::thread::ThreadId>();

        let task = background.spawn_dedicated(move |executor| async move {
            executor
                .spawn(async move {
                    // Blocking on `recv` is normally wrong inside an
                    // executor, but the dedicated thread is exclusive to
                    // this session, so blocking the only future on it is
                    // fine — this is the property `spawn_dedicated` is
                    // designed to provide.
                    gate_rx
                        .recv()
                        .expect("门控发送者在子任务恢复前被丢弃");
                    child_done_tx
                        .send(std::thread::current().id())
                        .expect("child_done 接收器被丢弃");
                })
                .detach();
            // Root finishes here. The detached child must keep the
            // dedicated thread alive until it completes.
        });

        futures::executor::block_on(task);

        // Negative assertion: the child has not finished, because the gate
        // hasn't been released yet.
        assert!(
            child_done_rx
                .recv_timeout(Duration::from_millis(50))
                .is_err(),
            "分离的子任务在释放前已完成"
        );

        // Release the gate. The detached child should now complete on the
        // dedicated thread.
        gate_tx.send(()).expect("门控接收器被丢弃");

        let child_thread_id = child_done_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("门控释放后,分离的子任务未能完成");
        assert_ne!(
            child_thread_id,
            std::thread::current().id(),
            "分离的子任务在测试线程上运行,而不是在专用线程上"
        );
    }
}
