use std::net::SocketAddr;

use anyhow::{Context, Result};
use demo_async::{Action, SERVER_ADDR};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    signal,
};

use crate::state::AppState;

mod state {
    use demo_async::{Action, ActionFailure, ActionResponse, ActionSuccess};
    use std::sync::{Arc, Mutex};

    #[derive(Debug, Clone)]
    pub(super) struct AppState {
        inner: Arc<Mutex<StateInner>>,
    }

    impl AppState {
        pub(super) fn new() -> Self {
            Self {
                inner: Arc::new(Mutex::new(StateInner::new())),
            }
        }

        pub(super) fn handle_action(&mut self, action: Action) -> ActionResponse {
            let mut lock = self.inner.lock().unwrap();
            match action {
                Action::Start(v) => {
                    if matches!(lock.state, WorkingState::Working(_)) {
                        return ActionResponse::Failure(ActionFailure::AlreadyStarted);
                    }

                    lock.state = WorkingState::Working(v);
                    return ActionResponse::Success(ActionSuccess::Done);
                }
                Action::Add(v) => {
                    lock.state = WorkingState::Working(v);
                    return ActionResponse::Success(ActionSuccess::Done);
                }
                Action::Remove(v) => {
                    if lock.state == WorkingState::Working(v) {
                        lock.state = WorkingState::Stopped;
                        return ActionResponse::Success(ActionSuccess::Done);
                    }
                    return ActionResponse::Failure(ActionFailure::NotRunning(v));
                }
                Action::Pause => match lock.state {
                    WorkingState::Working(v) => {
                        lock.state = WorkingState::Paused(v);
                        return ActionResponse::Success(ActionSuccess::Done);
                    }
                    WorkingState::Paused(_) => {
                        return ActionResponse::Failure(ActionFailure::AlreadyPaused);
                    }
                    WorkingState::Stopped => {
                        return ActionResponse::Failure(ActionFailure::AlreadyStopped);
                    }
                },
                Action::Stop => {
                    if lock.state == WorkingState::Stopped {
                        return ActionResponse::Failure(ActionFailure::AlreadyStopped);
                    }
                    return ActionResponse::Success(ActionSuccess::Done);
                }
                Action::Query => {
                    let resp_state = match lock.state {
                        WorkingState::Working(v) => ActionSuccess::Running(v),
                        WorkingState::Paused(v) => ActionSuccess::Paused(v),
                        WorkingState::Stopped => ActionSuccess::Stopped,
                    };

                    return ActionResponse::Success(resp_state);
                }
            }
        }
    }

    #[derive(Debug)]
    struct StateInner {
        state: WorkingState,
    }

    impl StateInner {
        fn new() -> Self {
            Self {
                state: WorkingState::Stopped,
            }
        }
    }

    #[derive(Debug, PartialEq, Eq)]
    enum WorkingState {
        Working(usize),
        Paused(usize),
        Stopped,
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let listener = TcpListener::bind(SERVER_ADDR)
        .await
        .context("failed to bind server addr")?;

    let app_state = state::AppState::new();

    // Listen for exits.
    tokio::spawn(async move {
        signal::ctrl_c()
            .await
            .expect("failed to check for ctrl c signal");

        println!("received ctrl c, exit now");
        std::process::exit(0);
    });

    loop {
        let (socket, client_addr) = listener.accept().await.unwrap();
        println!("[accept] start client_addr: {}", client_addr);
        let state = app_state.clone();

        tokio::spawn(handle_request(socket, client_addr, state));
    }
}

async fn handle_request(mut socket: TcpStream, addr: SocketAddr, mut state: AppState) {
    let mut buf = vec![0; 1024];
    loop {
        match socket.read(&mut buf).await {
            Ok(0) => {
                println!("[client={addr:?}] read buffer ended, done");
                return;
            }
            Ok(n) => match serde_json::from_slice::<Action>(&buf[..n]) {
                Ok(v) => {
                    let resp_data = state.handle_action(v);
                    println!("[client={addr:?}] {resp_data:?}");
                    let _ = socket
                        .write_all(serde_json::to_vec(&resp_data).unwrap().as_slice())
                        .await;
                    return;
                }
                Err(e) => {
                    let _ = socket
                        .write_all(format!("error: invalid action data: {e:?}").as_bytes())
                        .await;
                    break;
                }
            },
            Err(e) => {
                eprintln!("[client={addr:?}] ERROR: failed to read data: {e:?}");
                return;
            }
        }
    }
}
