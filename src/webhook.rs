use ipinfo::{IpInfo, IpInfoConfig};
use isahc::{AsyncReadResponseExt, HttpClient, Request};
use serde_json::Value;
use webhook::{client::WebhookClient, models::Message};

use crate::database::{Client, Connection, Database};

#[derive(Clone)]
pub struct SummaryWebhook {
    url: String,
    pub message_id: u64,
    client: HttpClient,
    ipinfo: String,
}
impl SummaryWebhook {
    pub async fn new(
        url: String,
        message_id: Option<u64>,
        database: Database,
        ipinfo: String,
    ) -> anyhow::Result<Self> {
        let client = HttpClient::new()?;
        let id = if let Some(id) = message_id {
            id
        } else {
            let req = Request::post(&format!("{}?wait=true", url.trim_end_matches("/")))
            .header("Content-Type", "application/json")
            // this is the best way to handle rate limits:
            .body(serde_json::to_string(&gen_summmary_message(
                database.clone(),
            ))?)?;
            let mut resp = client
                .send_async(
                    req
                )
                .await?;
            let json: Value = resp.json().await?;
            json["id"]
                .as_str()
                .expect("bad id in response from discord when creating webhook, what the fuck")
                .parse()
                .expect("apparently the id from the response is not a number (WTF??)")
        };

        Ok(Self {
            url,
            message_id: id,
            client,
            ipinfo,
        })
    }

    pub async fn update(&self, database: Database) -> anyhow::Result<()> {
        self.client
            .send_async(
                Request::patch(&format!(
                    "{}/messages/{}",
                    self.url.clone(),
                    self.message_id
                ))
                .header("Content-Type", "application/json")
                // this is the best way to handle rate limits:
                .header("x-pls-no-rate-limit", "owo")
                .body(serde_json::to_string(&gen_summmary_message(
                    database.clone(),
                ))?)?,
            )
            .await?;
        Ok(())
    }
}
fn gen_summmary_message(database: Database) -> Message {
    let mut m = Message::new();
    m.embed(|e| {
        e.title("Clients");
        for client in database.data.clone() {
            e.field(
                &format!("`{}`", &client.ip),
                &format!(
                    "Connections: `{}`{}",
                    client.connections.len(),
                    if client
                        .connections
                        .iter()
                        .filter(|con| con.server_responded == true)
                        .collect::<Vec<&Connection>>()
                        .len()
                        > 0
                    {
                        ", Server responded (valid ping?)"
                    } else {
                        ", Server did not respond (port scanner?)"
                    }
                ),
                false,
            );
        }
        e
    });
    m
}
#[derive(Clone)]
pub struct ConWebhook {
    url: String,
    ipinfo: String,
}
impl ConWebhook {
    pub fn new(url: String, ipinfo: String) -> Self {
        Self { url, ipinfo }
    }
    pub async fn handle_connect(&self, con: Connection, client: Client) -> anyhow::Result<()> {
        let cons = client
            .connections
            .iter()
            .filter(|&c| *c != con)
            .collect::<Vec<&Connection>>()
            .len();
        let ipinfo_res: Result<Value, serde_json::Error> = isahc::get_async(format!("https://ipinfo.com/{}?token={}", client.ip.clone(), &self.ipinfo)).await?.json().await;
        WebhookClient::new(&self.url)
            .send(|m| {
                m.embed(|e| {
                    e.title("New connection");
                    if cons == 0 {
                        e.description("**First Connection!**");
                    } else {
                        e.description(&format!("connected {} times before", cons));
                    };
                    e.field(
                        "ip",
                        &format!(
                            "(`{}`)[https://ipinfo.io/{}] | {}",
                            client.ip,
                            client.ip,
                            if let Ok(json_res) = &ipinfo_res {
                                json_res["org"].as_str().unwrap_or("d")
                            } else {
                                "a"
                            }
                        ),
                        false,
                    )
                    .field(
                        "duration connected",
                        &con.duration_connected.to_string(),
                        false,
                    )
                    .field(
                        "server responded",
                        &con.server_responded.to_string(),
                        false,
                    )
                })
            })
            .await
            .expect("failed to send webhook");
        Ok(())
    }
}
