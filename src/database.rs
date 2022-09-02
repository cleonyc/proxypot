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
use std::{path::PathBuf, str::FromStr};


use isahc::{AsyncReadResponseExt, Request};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::{OffsetDateTime};
use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncReadExt, AsyncWriteExt, BufReader},
};
use uuid::Uuid;

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
    pub async fn handle_ping(&mut self, ip: &str, target: String) -> anyhow::Result<(Client, Ping)>{
        let client = match self.data.iter_mut().find(|client| client.ip == ip) {
            Some(client) => {client},
            None => {
                let client = Client {
                    ip: ip.to_string(),
                    logins: vec![],
                    pings: vec![],
                    ipinfo: get_ipinfo(&ip).await.unwrap_or("".to_string())
                };
                self.data.push(client);
                self.data.iter_mut().find(|c| c.ip == ip).unwrap()
            },
        };
        let ping = Ping {
            target,
            time: OffsetDateTime::now_utc()
        };
        client.pings.push(ping.clone());
        let cloned = client.clone();
        self.save().await?;
        Ok((cloned, ping))
    }
    pub async fn handle_login(&mut self, ip: &str, mut username: String) -> anyhow::Result<(Client, Login)> {
        let mut resp = isahc::get_async(format!("https://api.mojang.com/users/profiles/minecraft/{}", username)).await?;
        let uuid = if resp.status() != 200 {
            username += " [Cracked]";
            None
        } else {
            let json: Value = resp.json().await?;
            match json["id"].as_str() {
                Some(uuid) => {Some(uuid::Uuid::from_str(uuid)?)},
                None => {
                    username += " [Cracked]";
                    None
                },
            }
        };
        let client = match self.data.iter_mut().find(|client| client.ip == ip) {
            Some(client) => {client},
            None => {
                let client = Client {
                    ip: ip.to_string(),
                    logins: vec![],
                    pings: vec![],
                    ipinfo: get_ipinfo(&ip).await.unwrap_or("".to_string())
                };
                self.data.push(client);
                self.data.iter_mut().find(|c| c.ip == ip).unwrap()
            },
        };
        let login = Login {
            username,
            uuid,
            time: OffsetDateTime::now_utc(),
        };
        client.logins.push(login.clone());
        let cloned = client.clone();

        self.save().await?;
        Ok((cloned, login))
    }
}
impl Default for Database {
    fn default() -> Self {
        Self { path: "./database.json".into(), data: vec!() }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Client {
    pub logins: Vec<Login>,
    pub pings: Vec<Ping>,
    pub ip: String,
    pub ipinfo: String
}
#[derive(Serialize, Deserialize, Clone)]
pub struct Login {
    pub username: String,
    pub uuid: Option<Uuid>,
    pub time: OffsetDateTime
}
#[derive(Serialize, Deserialize, Clone)]

pub struct Ping {
    pub time: OffsetDateTime,
    pub target: String,
}
async fn get_ipinfo(ip: &str) -> anyhow::Result<String> {
    let request = Request::get(format!("https://ipinfo.io/widget/demo/{}", ip)).header("referer", "https://ipinfo.io/").body("")?;
    let mut r = isahc::send_async(request).await?;
    let json: Value = r.json().await?;
    Ok(format!("Company: {}, Location: {}, {}", json["data"]["company"]["name"].as_str().unwrap_or("Unknown"), json["data"]["region"].as_str().unwrap_or("Unknown"), json["data"]["country"].as_str().unwrap_or("Unknown")))
}

#[tokio::test]
async fn test_get_ipinfo() {
    let ipinfo = get_ipinfo("89.45.224.142").await.unwrap(); 
    assert_eq!(ipinfo, "Company: M247 LTD New York Infrastructure, Location: New York, US".to_string())
}