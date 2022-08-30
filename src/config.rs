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
use std::path::{PathBuf, Path};

use serde::{Serialize, Deserialize};
use tokio::{fs::{File, OpenOptions}, io::{BufReader, AsyncReadExt, AsyncWriteExt}};

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