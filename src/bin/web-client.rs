use anyhow::{Context, Result};
use demo_async::{Action, ActionResponse, SERVER_ADDR};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

const USAGE: &'static str = r#"Usage:
start <id>
add <id>
remove <id>
pause
stop
query
"#;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        println!("{}", USAGE);
        return Ok(());
    }

    let socket = TcpStream::connect(SERVER_ADDR)
        .await
        .context("failed to connect to server")?;

    let id = args.get(2).and_then(|x| str::parse(x.as_str()).ok());

    let payload = match args[1].as_str() {
        "start" if id.is_some() => Action::Start(id.unwrap()),
        "add" if id.is_some() => Action::Add(id.unwrap()),
        "remove" if id.is_some() => Action::Remove(id.unwrap()),
        "pause" => Action::Pause,
        "stop" => Action::Stop,
        "query" => Action::Query,
        _ => {
            println!("{}", USAGE);
            return Ok(());
        }
    };

    send_action(socket, payload).await?;

    Ok(())
}

async fn send_action(mut socket: TcpStream, action: Action) -> Result<()> {
    let (mut rd, mut wr) = socket.split();

    if let Err(e) = wr.write_all(&serde_json::to_vec(&action).unwrap()).await {
        eprintln!("failed to send action to server: {e:?}");
        return Ok(());
    }

    let mut buf = vec![0; 128];

    loop {
        let n = rd.read(&mut buf).await?;
        if n == 0 {
            println!("done!");
            break;
        }

        let resp: ActionResponse = serde_json::from_slice(&buf[0..n]).unwrap();
        println!("got echo: {:?}", resp);
    }

    Ok(())
}
