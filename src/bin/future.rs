use std::{future::Future, task::Poll, time::Duration};

use tokio::time::Instant;

struct Delay {
    when: Instant,
}

impl Future for Delay {
    type Output = &'static str;
    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        if Instant::now() >= self.when {
            println!("hello future world!");
            Poll::Ready("DONE")
        } else {
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

#[tokio::main]
async fn main() {
    let when = Instant::now() + Duration::from_millis(10);
    let future = Delay { when };

    let out = future.await;
    println!("GET RESULT: {out}");
}
