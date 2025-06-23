use bytes::Bytes;
use mini_redis::client;
use tokio::sync::{mpsc, oneshot};

type Responder<T> = oneshot::Sender<mini_redis::Result<T>>;

#[derive(Debug)]
enum Command {
    Get {
        key: String,
        resp: Responder<Option<Bytes>>,
    },
    Set {
        key: String,
        val: Bytes,
        resp: Responder<()>,
    },
}

#[tokio::main]
async fn main() {
    let (tx, mut rx) = mpsc::channel::<Command>(32);
    let tx2 = tx.clone();

    // Command sender 1.
    let t1 = tokio::spawn(async move {
        // The "callback" through which task manager sends command result back to sender.
        // So the `resp` is the `sender` of oneshot channel and sender holds the `receiver`
        // to get command result sent by task manager.
        let (resp_tx, resp_rx) = oneshot::channel();
        let cmd = Command::Get {
            key: "foo".to_string(),
            resp: resp_tx,
        };
        tx.send(cmd).await.unwrap();

        let res = resp_rx.await.unwrap();
        println!("sender 1 result: {:?}", res);
    });

    // Command sender 2.
    let t2 = tokio::spawn(async move {
        let (resp_tx, resp_rx) = oneshot::channel();
        let cmd = Command::Set {
            key: "foo".to_string(),
            val: "bar".into(),
            resp: resp_tx,
        };
        tx2.send(cmd).await.unwrap();

        let res = resp_rx.await.unwrap();
        println!("sender 2 result: {:?}", res);
    });

    // Managing all tasks.
    //
    // Holding the client instance, receive commands from all senders and do the underlying work.
    let manager = tokio::spawn(async move {
        let mut client = client::connect("127.0.0.1:6379").await.unwrap();

        while let Some(cmd) = rx.recv().await {
            use Command::*;
            match cmd {
                Get { key, resp } => {
                    // Result of replying the command execute result to original sender.
                    let resp_result = match client.get(&key).await {
                        Ok(v) => resp.send(Ok(v)),
                        Err(e) => resp.send(Err(e)),
                    };

                    if let Err(e) = resp_result {
                        println!("manager failed to reply Get command: {e:?}");
                    }
                }
                Set { key, val, resp } => {
                    let resp_result = match client.set(&key, val).await {
                        Ok(_) => resp.send(Ok(())),
                        Err(e) => resp.send(Err(e)),
                    };
                    if let Err(e) = resp_result {
                        println!("manager failed to reply Set command: {e:?}");
                    }
                }
            }
        }
    });

    // Waiting for all works done.
    t1.await.unwrap();
    t2.await.unwrap();
    manager.await.unwrap();
}
