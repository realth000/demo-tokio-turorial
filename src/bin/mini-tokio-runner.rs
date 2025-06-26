use std::time::Duration;

use demo_async::mini_tokio::{self, spawn};
use mini_tokio::MiniTokio;

fn main() {
    let mini_tokio = MiniTokio::new();

    mini_tokio.spawn(async {
        spawn(async {
            MiniTokio::delay("polling world", Duration::from_millis(1000)).await;
            println!("world!");
        });

        spawn(async {
            println!("hello");
        });

        MiniTokio::delay("waiting for finish", Duration::from_millis(2000)).await;
        std::process::exit(0);
    });

    mini_tokio.run();
}
