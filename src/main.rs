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

use crate::packet::get_all_packets;
use bytes::BytesMut;
use clap::Parser;
use futures::{FutureExt, StreamExt};
use logger::Logger;
use std::collections::VecDeque;
use std::error::Error;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration as StdDuration;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use tokio_util::codec::{BytesCodec, FramedRead};

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
    let split = peer.split(':').collect::<Vec<&str>>();
    let reconnects = logger
        .read()
        .await
        .database
        .data
        .iter()
        .find(|c| c.ip == split[0])
        .map(|c| c.logins.len())
        .unwrap_or(0);

    println!("reconnects: {}", reconnects);

    let timeout_seconds = if reconnects == 0 {
        // first join, 10-30 minutes
        (rand::random::<f64>() * 60.0 * 20.0) as u64 + 10 * 60
    } else {
        // they've joined before, 10-130s
        (rand::random::<f64>() * 60.0 * 2.0) as u64 + 10
    };

    // ping is random between 0 and reconnects*1000ms
    let simulated_ping: u64 = (rand::random::<f32>() * (reconnects as f32) * 1000.0) as u64;
    println!("simulated_ping: {}ms", simulated_ping);

    let mut outbound = match TcpStream::connect(proxy_addr).await {
        Ok(o) => o,
        Err(_) => return Ok(()),
    };
    let (mut read_inbound, mut write_inbound) = inbound.split();
    let (read_outbound, mut write_outbound) = outbound.split();

    let client_to_server = async {
        // uses read_inbound and write_outbound

        // println!("start");
        // wait lag before actually letting them connect
        tokio::time::sleep(StdDuration::from_millis(simulated_ping)).await;

        for packet in get_all_packets(&mut read_inbound, &mut write_outbound).await {
            let logger = logger.clone();
            let ip = split[0].to_string();
            let p = packet.clone();
            tokio::spawn(async move { logger.write().await.handle_connect(p, &ip).await });
        }

        // copy packets from ri to wo
        let packet_queue: VecDeque<(BytesMut, tokio::time::Instant)> = VecDeque::new();
        let packet_queue = Arc::new(std::sync::Mutex::new(packet_queue));

        let task_packet_queue = packet_queue.clone();

        // read from the queue and write to write_outbound
        let read_from_queue = async move {
            loop {
                // check if there's something in the packet queue every simulated_ping ms
                tokio::time::sleep(StdDuration::from_millis(simulated_ping)).await;
                // if there is, wait until the packet is ready to be sent and send it
                loop {
                    let queue_front = packet_queue.lock().unwrap().pop_front();
                    if let Some((bytes, sent_at)) = &queue_front {
                        let sending_at = *sent_at + StdDuration::from_millis(simulated_ping);
                        tokio::time::sleep_until(sending_at).await;
                        if write_outbound.write_all(bytes.as_ref()).await.is_err() {
                            break;
                        }
                    } else {
                        break;
                    }
                }
            }
        };

        let write_to_queue = async move {
            let mut framed = FramedRead::new(read_inbound, BytesCodec::new());
            while let Some(message) = framed.next().await {
                match message {
                    Ok(bytes) => {
                        task_packet_queue
                            .lock()
                            .unwrap()
                            .push_back((bytes, tokio::time::Instant::now()));
                    }
                    Err(_) => break,
                }
            }
        };

        tokio::join!(read_from_queue, write_to_queue);
    };

    let server_to_client = async {
        let mut outbound_framed = FramedRead::new(read_outbound, BytesCodec::new());
        // copy packets from ro to wi
        while let Some(message) = outbound_framed.next().await {
            match message {
                Ok(bytes) => {
                    if write_inbound.write_all(&bytes).await.is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }

        write_inbound.shutdown().await
    };

    let _ = tokio::try_join!(
        tokio::time::timeout(StdDuration::from_secs(timeout_seconds), client_to_server),
        tokio::time::timeout(StdDuration::from_secs(timeout_seconds), server_to_client)
    );

    // so it times out
    tokio::time::sleep(StdDuration::from_millis(5 * 60 * 1000)).await;

    Ok(())
}
