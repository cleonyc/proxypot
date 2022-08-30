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
use std::path::{PathBuf};

use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};
use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncReadExt, AsyncWriteExt, BufReader},
};

#[derive(Clone)]
pub struct Database {
    path: PathBuf,
    pub data: Vec<Client>,
}
impl Database {
    pub async fn open(path: PathBuf) -> anyhow::Result<Self> {
        let mut file = File::open(&path).await?;
        let mut buf_reader = BufReader::new(&mut file);
        let mut content = String::new();
        buf_reader.read_to_string(&mut content).await?;
        if content == "" {
            content = "[]".to_string()
        }
        let data: Vec<Client> = serde_json::from_str(&content)?;
        Ok(Self { path, data })
    }
    pub async fn save(&self) -> anyhow::Result<()> {
        let mut file = OpenOptions::new().write(true).truncate(true).create(true).open(&self.path).await?;
        file
            .write_all(serde_json::to_string(&self.data)?.as_bytes())
            .await?;
        Ok(())
    }
    pub async fn handle_connect(&mut self, connection: Connection, ip: String) -> anyhow::Result<Client> {
        let mut ret_client = None;
        for client in self.data.iter_mut() {
            if client.ip == ip {
                client.connections.push(connection.clone());
                ret_client = Some(client.clone());
                break;
            }
        }
        if ret_client.is_none() {
            ret_client = Some(Client {
                connections: vec![connection],
                last_connected: OffsetDateTime::now_utc(),
                ip
            });
            self.data.push(ret_client.clone().unwrap())
        }
        self.save().await?;
        Ok(ret_client.unwrap())
    }
}
impl Default for Database {
    fn default() -> Self {
        Self { path: "./database.json".into(), data: vec!() }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Client {
    pub connections: Vec<Connection>,
    pub last_connected: OffsetDateTime,
    pub ip: String,
}
#[derive(Serialize, Deserialize, Clone, PartialEq)]

pub struct Connection {
    pub duration_connected: Duration,
    pub server_responded: bool,
    pub port: u16, 
}