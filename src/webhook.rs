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
use isahc::{AsyncReadResponseExt, HttpClient, Request};
use serde_json::Value;
use webhook::{client::WebhookClient, models::Message};

use crate::database::{Client, Database, Login, Ping};

#[derive(Clone)]
pub struct SummaryWebhook {
    url: String,
    pub message_ids: Vec<u64>,
    client: HttpClient,
}
impl SummaryWebhook {
    pub async fn new(
        url: String,
        message_ids: Vec<u64>,
        database: Database,
    ) -> anyhow::Result<Self> {
        let client = HttpClient::new()?;
        let mut ret = Self {
            url,
            message_ids: message_ids,
            client,
        };
        ret.update(database.clone()).await?;

        Ok(ret)
    }

    pub async fn update(&mut self, database: Database) -> anyhow::Result<()> {
        let gen_messages = gen_summmary_messages(database.clone());
        if gen_messages.len() != self.message_ids.len() {
            println!("a");
            for index in self.message_ids.len()..gen_messages.len() {
                println!("i: {index}");
                let msg = &gen_messages[index];
                let req = Request::post(&format!("{}?wait=true", self.url.trim_end_matches("/")))
                    .header("Content-Type", "application/json")
                    .body(serde_json::to_string(&msg)?)?;
                let mut resp = self.client.send_async(req).await?;
                let json: Value = resp.json().await?;
                let id: u64 = json["id"]
                    .as_str()
                    .expect("bad id in response from discord when creating webhook, what the fuck")
                    .parse()
                    .expect("apparently the id from the response is not a number (WTF??)");
                self.message_ids.push(id);
            }
        } else {
         
            println!("B");
        }
        for (index, msg) in gen_summmary_messages(database.clone()).iter().enumerate() {
            self.client
                .send_async(
                    Request::patch(&format!(
                        "{}/messages/{}",
                        self.url.clone(),
                        self.message_ids[index]
                    ))
                    .header("Content-Type", "application/json")
                    // this is the best way to handle rate limits:
                    .header("x-pls-no-rate-limit", "owo")
                    .body(serde_json::to_string(&msg)?)?,
                )
                .await?;
        }
        Ok(())
    }
}
fn gen_summmary_messages(database: Database) -> Vec<Message> {
    let mut ret = vec![];
    for chunk_num in 0..(database.data.clone().len() / 25 + 1) {
        let mut m = Message::new();
        m.embed(|e| {
            if chunk_num == 0 {
                e.title("Clients");
            }
            for client in &mut database.data.clone()[..((25 * (chunk_num + 1)) - (25 - database.data.len() % 25))] {
                e.field(
                        &format!("`{}`", &client.ip),
                    &format!(
                        "Pings: `{}` (Last: {}), Logins: `{}` (Last: {}), Con: {}",
                        client.pings.len(),
                        if client.pings.len() == 0 {
                            "N/A".to_string()
                        } else {
                            format!(
                                "<t:{}:R>",
                                client
                                    .pings
                                    .iter()
                                    .reduce(|a, b| if a.time > b.time { b } else { a })
                                    .unwrap()
                                    .time
                                    .unix_timestamp()
                            )
                        },
                        client.logins.len(),
                        if client.logins.len() == 0 {
                            "N/A".to_string()
                        } else {
                            format!(
                                "<t:{}:R>",
                                client
                                    .logins
                                    .iter()
                                    .reduce(|a, b| if a.time > b.time { b } else { a })
                                    .unwrap()
                                    .time
                                    .unix_timestamp()
                            )
                        },
                        client.ipinfo
                    ),
                    true,
                );
            }
            e
        });
        ret.push(m)
    }
    ret
}
#[derive(Clone)]
pub struct ConWebhook {
    url: String,
}
impl ConWebhook {
    pub fn new(url: String) -> Self {
        Self { url }
    }
    pub async fn handle_login(&self, client: Client, login: Login) -> anyhow::Result<()> {
        WebhookClient::new(&self.url)
            .send(|m| {
                m.content(
                    &format!(
                        "`{}` joined the server
{} | {}
{}
                ",
                        login.username,
                        pretty_ip(&client.ip),
                        client.ipinfo,
                        if client.logins.len() == 1 {
                            "**First Login**".to_string()
                        } else {
                            format!("Previous logins: `{}`", client.logins.len())
                        }
                    )
                    .trim(),
                )
            })
            .await
            .expect("failed to send webhook");
        Ok(())
    }
    pub async fn handle_ping(&self, client: Client, ping: Ping) -> anyhow::Result<()> {
        WebhookClient::new(&self.url)
            .send(|m| m.content(&format!("Ping from {}, Con: {}, Target: {}", pretty_ip(&client.ip), client.ipinfo, ping.target)))
            .await
            .unwrap();
        Ok(())
    }
}

fn pretty_ip(ip: &str) -> String {
    format!("[`{}`](https://ipinfo.io/{})", ip, ip)
}
