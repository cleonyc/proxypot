use std::path::PathBuf;

use webhook::{client::WebhookClient, models::Message};

use crate::{
    config::Config,
    database::{Connection, Database, Client}, webhook::{SummaryWebhook, ConWebhook},
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
        let summary_webhook = SummaryWebhook::new(config.clone().summary_webhook_url, config.clone().summary_message_id, database.clone(), config.ipinfo_token.clone()).await?;
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
