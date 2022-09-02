// minecraft honeypot does honeypot things for minecraft and proxies which is cool
// Copyright (C) 2022 cleonyc

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

mod config;
mod database;
mod logger;
mod packet;
mod webhook;







use clap::Parser;
use futures::FutureExt;
use logger::Logger;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration as StdDuration;

use tokio::io::AsyncWriteExt;
use tokio::io::{self};

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;

use std::error::Error;

use crate::packet::{get_all_packets};

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
pub struct Cli {
    /// Config file
    #[clap(value_parser)]
    config: PathBuf,
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
    let mut join_handles = vec![];
    while let Ok((inbound, _)) = listener.accept().await {
        let transfer = transfer(inbound, sv.clone(), wrapped.clone()).map(|r| r.unwrap());
        join_handles.push(tokio::spawn(transfer));
    }
    for jh in join_handles {
        jh.await.unwrap();
    }

    Ok(())
}

async fn transfer(
    mut inbound: TcpStream,
    proxy_addr: String,
    logger: Arc<RwLock<Logger>>,
) -> Result<(), Box<dyn Error>> {
    let peer = inbound.peer_addr()?.to_string();
    let split = peer.split(":").collect::<Vec<&str>>();
    let mut timeout = (rand::random::<f64>() * 60.0 * 20.0) as u64 + 10 * 60;
    if let Some(client)  = logger.read().await.database.data.iter().find(|c| c.ip == split[0]) {
        if client.logins.len() > 1 {
            timeout = (rand::random::<f64>() * 60.0 * 2.0) as u64 + 10;
        }
    }
    let mut outbound = match TcpStream::connect(proxy_addr).await {
        Ok(o) => o,
        Err(_) => return Ok(()),
    };
    let (mut ri, mut wi) = inbound.split();
    let (mut ro, mut wo) = outbound.split();
    let mut detected_packets = vec![];
    let client_to_server = async {
        // println!("start");
        for packet in get_all_packets(&mut ri,&mut wo).await {
            let lc  = logger.clone();
            let ip = split[0].clone().to_string();
            let p = packet.clone();
            detected_packets.push(tokio::spawn(async move {
                lc.write().await.handle_connect(p, &ip).await
            }));
            // println!("packet: {:?}", packet)
        }
        io::copy(&mut ri, &mut wo).await?;
        let r = wo.shutdown().await;
        r
    };

    
    let server_to_client = async {
        io::copy(&mut ro, &mut wi).await?;
        wi.shutdown().await
    };
    match tokio::try_join!(
        tokio::time::timeout(StdDuration::from_secs(timeout), client_to_server),
        tokio::time::timeout(StdDuration::from_secs(timeout), server_to_client)
    ) {
        Ok(_) => {}
        Err(_) => {}
    };
    // for jh in detected_packets {
    //     jh.await??;
    // }
    tokio::time::sleep(StdDuration::from_secs(60 * 5)).await;
    Ok(())
}
