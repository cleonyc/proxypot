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
use std::path::PathBuf;



use crate::{
    config::Config,
    database::{Connection, Database}, webhook::{SummaryWebhook, ConWebhook},
};
pub struct Logger {
    pub database: Database,
    pub config: Config,
    pub summary_webhook: SummaryWebhook,
    pub webhook: ConWebhook
}
impl Logger {
    pub async fn new(config_path: PathBuf) -> anyhow::Result<Self> {
        let mut config = Config::open(config_path.clone())
            .await
            .expect("Invalid config file specified");
        let database  = Database::open(config.clone().database).await.unwrap_or_default();
        database.save().await?;
        let summary_webhook = SummaryWebhook::new(config.clone().summary_webhook_url, config.clone().summary_message_id, database.clone()).await?;
        if config.summary_message_id.is_none() {
            config.summary_message_id = Some(summary_webhook.message_id);
            config.save(config_path).await?;
        }
        let webhook = ConWebhook::new(config.clone().webhook_url, config.clone().ipinfo_token);
        
        Ok(Self {
            database,
            config,
            summary_webhook,
            webhook
        })
    }
    pub async fn handle_connect(&mut self, ip: String, con: Connection) -> anyhow::Result<()>{
        let client = self.database.handle_connect(con.clone(), ip).await?;
        self.webhook.handle_connect(con.clone(), client.clone()).await?;
        self.summary_webhook.update(self.database.clone()).await?;
        Ok(())
    }
    
}
