
mod config;
mod logger;
mod database;
mod webhook;

use clap::Parser;
use database::Connection;
use logger::Logger;
use time::OffsetDateTime;
use tokio::io::{self, AsyncReadExt};
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration as StdDuration;
use futures::FutureExt;
use std::env;
use std::error::Error;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
pub struct Cli {
    /// Config file
    #[clap(value_parser)]
    config: PathBuf    
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    let logger = Logger::new(cli.config).await?;
    println!("Listening on: {}", logger.config.bind);
    let sv = logger.config.server.clone();
    println!("Proxying to: {}", sv.clone());

    let listener = TcpListener::bind(logger.config.bind.clone()).await?;
    let wrapped = Arc::new(RwLock::new(logger));
    let mut join_handles = vec!();
    while let Ok((inbound, _)) = listener.accept().await {
        let transfer = transfer(inbound, sv.clone(), wrapped.clone()).map(|r| {
            r.unwrap()
        });
        join_handles.push(tokio::spawn(transfer));
    }
    for jh in join_handles {
        jh.await.unwrap();
    }

    Ok(())
}

async fn transfer(mut inbound: TcpStream, proxy_addr: String, logger:  Arc<RwLock<Logger>>) -> Result<(), Box<dyn Error>> {
    let start = OffsetDateTime::now_utc();
    let peer = inbound.peer_addr()?.to_string();
    let split = peer.split(":").collect::<Vec<&str>>();
    let port: u16 = if split.len() > 1 {split[1].parse()?} else {0};
    let mut timeout = 15;
    if let Some(client)  = logger.read().await.database.data.iter().find(|c| c.ip == split[0]) {
        if client.connections.len() > 10 {
            timeout = 1;
        }
    }
    let mut outbound = match TcpStream::connect(proxy_addr).await {
        Ok(o) => {o},
        Err(_) => {return Ok(())},
    };
    let (mut ri, mut wi) = inbound.split();
    let (mut ro, mut wo) = outbound.split();
    let mut finish = OffsetDateTime::now_utc();

    let client_to_server = async {
        io::copy(&mut ri, &mut wo).await.expect("copy");
        let r = wo.shutdown().await;
        finish = OffsetDateTime::now_utc();
        r
    };
    let mut sv_sent_data = false;

    let server_to_client = async {
        let mut buf = [0; 1];
        while let Ok(_) = ro.read(&mut buf).await {
            match wi.write_all(&mut buf).await {
                Ok(_) => {},
                Err(_) => {},
            };
            if !sv_sent_data {sv_sent_data = true};
        }
        wi.shutdown().await
        
    };
    match tokio::try_join!(tokio::time::timeout(StdDuration::from_secs(timeout),client_to_server), tokio::time::timeout(StdDuration::from_secs(timeout), server_to_client)) {
        Ok(_) => {},
        Err(_) => {},
    };
    logger.write().await.handle_connect(split[0].to_string(), Connection {
        duration_connected: finish - start,
        server_responded: sv_sent_data,
        port
    }).await.expect("logger");
    tokio::time::sleep(StdDuration::from_secs(60*5)).await;
    Ok(())
}