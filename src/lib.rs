pub const SERVER_ADDR: &'static str = "127.0.0.1:34567";

mod connection;
pub use connection::Connection;
use serde::{Deserialize, Serialize};
pub mod mini_tokio;

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum Action {
    Start(usize),
    Add(usize),
    Remove(usize),
    Pause,
    Stop,
    Query,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum ActionResponse {
    Success(ActionSuccess),
    Failure(ActionFailure),
}

#[derive(Debug, Deserialize, Serialize)]
pub enum ActionSuccess {
    Done,
    Running(usize),
    Paused(usize),
    Stopped,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum ActionFailure {
    AlreadyStarted,
    AlreadyPaused,
    AlreadyStopped,
    InvalidId(usize),
    NotRunning(usize),
}
