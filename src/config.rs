use std::path::{PathBuf, Path};

use serde::{Serialize, Deserialize};
use tokio::{fs::{File, OpenOptions}, io::{BufReader, AsyncReadExt, BufWriter, AsyncWriteExt}};

#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    pub webhook_url: String,
    pub summary_webhook_url: String,
    pub summary_message_id: Option<u64>,
    pub bind: String,
    pub server: String,
    pub database: PathBuf,
    pub ipinfo_token: String,
}
impl Config {
    pub async fn open(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let mut file = File::open(&path).await?;
        let mut buf_reader = BufReader::new(&mut file);
        let mut content = String::new();
        buf_reader.read_to_string(&mut content).await?;
        
        Ok(toml::from_str(&content)?)
    }
    pub async fn save(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let mut file = OpenOptions::new().write(true).truncate(true).open(path).await?;
        // let mut buf_writer = BufWriter::new(&mut file);
        file.write_all(toml::to_string_pretty(&self)?.as_bytes()).await?;
        Ok(())
    }
}