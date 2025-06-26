use std::{
    cell::RefCell,
    future::Future,
    sync::{mpsc, Arc, Mutex},
    task::Context,
    time::Duration,
};

use futures::{
    future::BoxFuture,
    task::{self, ArcWake},
};

// Used to track the current mini-tokio instance so that the `spawn` function is
// able to schedule spawned tasks.
thread_local! {
    static CURRENT: RefCell<Option<mpsc::Sender<Arc<Task>>>> =
        RefCell::new(None);
}

mod delay {
    use std::{
        future::Future,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc, Mutex,
        },
        task::{Poll, Waker},
        thread,
        time::{Duration, Instant},
    };

    /// Like `tokio::sleep`, async sleep for the given mount of time.
    ///
    /// After the duration passed, call `Waker` to complete it.
    ///
    /// This struct reaches `sleep` state by returning `Poll::Pending` state in sleep,
    /// and will be awaken by `waker` field once timeout.
    struct Delay {
        id: &'static str,
        when: Instant,
        waker: Option<Arc<Mutex<Waker>>>,

        /// Flag indicating we have spawned a thread to wake or not.
        ///
        /// If true, do not spawn another thread.
        in_counting: Mutex<AtomicBool>,
    }

    impl Delay {
        fn debug(&self, msg: &'static str) {
            println!(">>> Delay [{}]: {}", self.id, msg);
        }
    }

    impl Future for Delay {
        type Output = ();

        fn poll(
            mut self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Self::Output> {
            self.debug("polling...");

            // Check if timeout, is so, return ready state.
            if Instant::now() >= self.when {
                self.debug("ready");
                return Poll::Ready(());
            }

            // From here the future is not complete.

            // This function is a example showing how to implement `Future` trait.
            //
            // If `Waker` is available, check `self.waker != cx.waker()`, where
            // `will_wake` returns false, use the one in `cx`.
            //
            // Remember that `poll` may be called many times, in each round we update
            // `self.waker` from the one in `cx`.
            //
            // It is required to call poll on most recent waker.

            if let Some(waker) = &self.waker {
                // We have waker carried in `self`.
                let mut waker = waker.lock().unwrap();
                // `will_wake` check if `self.waker` and `cx.waker()` wakes the same task.
                // Use the one in `cx` if `self.waker` not so.
                if !waker.will_wake(cx.waker()) {
                    *waker = cx.waker().clone();
                }
            } else {
                // We do not have waker, use the one cloned from `cx.waker()`.
                let waker = Arc::new(Mutex::new(cx.waker().clone()));
                self.waker = Some(waker);
            }

            // Only spawn the new thread if never did that before.
            let in_counting = self.in_counting.lock().unwrap();

            if !in_counting.load(Ordering::Relaxed) {
                // Spawn new thread and update it.
                in_counting.store(true, Ordering::Relaxed);
                self.debug("start in_counting");

                // Clone and move.
                let when = self.when;
                // Clone and move.
                let waker = self.waker.clone();

                // Create a new id to move it into spawned thread.
                let id = self.id.to_string();

                // Spawn a thread to do the task as a timer.
                thread::spawn(move || {
                    let now = Instant::now();

                    if now < when {
                        println!(">>> Thread [{}]: start sleep", id);
                        thread::sleep(when - now);
                    }
                    println!(">>> Thread [{}]: sleep finished, awakening...", id);

                    // Timeout, wake the task, which calls `poll` again.
                    let waker = waker.unwrap();
                    waker.lock().unwrap().wake_by_ref();
                });
            } else {
                self.debug(">>> already in_counting <<<");
            }

            Poll::Pending
        }
    }

    pub(super) async fn delay(id: &'static str, dur: Duration) {
        let future = Delay {
            id,
            when: Instant::now() + dur,
            waker: None,
            in_counting: Mutex::new(AtomicBool::new(false)),
        };

        future.await;
    }
}

/// Like `tokio::spawn()`.
pub fn spawn<F>(future: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    CURRENT.with(|cell| {
        let borrow = cell.borrow();
        let sender = borrow.as_ref().unwrap();
        Task::spawn(future, sender);
    });
}

struct Task {
    /// Future owned by the task.
    ///
    /// `BoxFuture` is something implements `Future` and wrapped by `Box` and `Pin`.
    /// It also `Send`.
    future: Mutex<BoxFuture<'static, ()>>,

    /// Channel piping task into.
    executor: mpsc::Sender<Arc<Task>>,
}

impl Task {
    /// Run a task when the given `future`.
    ///
    /// This is a "static" function, utility function to instantiate a `Task` instance.
    ///
    /// * `future` is the work to do.
    /// * `sender` is channel handler passed from `MiniTokio` instance.
    fn spawn<F>(future: F, sender: &mpsc::Sender<Arc<Task>>)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        // Construct a `Task`.
        let task = Arc::new(Task {
            future: Mutex::new(Box::pin(future)),
            executor: sender.clone(),
        });

        // Run the task in place to start work.
        let _ = sender.send(task);
    }

    /// Do the future once.
    ///
    /// What the function does is sending `Self` to manager, the `MiniTokio`.
    fn poll(self: Arc<Self>) {
        // A `Waker` is a handle for waking up a task by notifying its executor that it
        // is ready to be run.
        let waker = task::waker(self.clone());

        // Context provide access to the waker.
        let mut cx = Context::from_waker(&waker);

        // Get the future.
        let mut future = self.future.try_lock().unwrap();

        // Run the `poll` method on `Future`.
        // Here we pass context as parameter so in the `poll` function waker is accessible.
        // Besides what inside `Waker` is a type implements `ArcWake`, in this example is
        // `Self`, the task, inside `poll` the future can run the `wake_by_ref` defined below.
        // Inside `wake_by_ref`, `executor` in `Self` send `Task` itself to channel, finally on the
        // otherside of channel is the `scheduled` in `MiniTokio`.
        let _ = future.as_mut().poll(&mut cx);
    }
}

impl ArcWake for Task {
    /// When this function is called, means calling [`Task::poll`].
    fn wake_by_ref(arc_self: &Arc<Self>) {
        let _ = arc_self.executor.send(arc_self.clone());
    }
}

pub struct MiniTokio {
    /// Tasks in channel are works that need to run.
    ///
    /// When a task is ready for do operation, or say it prepared to make process, it is sent in the
    /// channel's receiver side.
    scheduled: mpsc::Receiver<Arc<Task>>,

    /// All tasks to run in the future.
    sender: mpsc::Sender<Arc<Task>>,
}

impl MiniTokio {
    pub fn new() -> Self {
        let (sender, scheduled) = mpsc::channel();

        MiniTokio { scheduled, sender }
    }

    pub fn spawn<F>(&self, future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        Task::spawn(future, &self.sender);
    }

    pub async fn delay(id: &'static str, dur: Duration) {
        delay::delay(id, dur).await
    }

    pub fn run(&self) {
        CURRENT.with(|cell| {
            *cell.borrow_mut() = Some(self.sender.clone());
        });

        while let Ok(task) = self.scheduled.recv() {
            println!(">>> MiniTokio: do task");
            task.poll();
        }
    }
}
