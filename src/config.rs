use crate::APP_INFO;
use anyhow::Result;
use app_dirs::*;
use std::{fs::File, io::BufReader, path::PathBuf};

const CONFIG_FILE_NAME: &str = "config.json";

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub default_project_key: String,
    pub filter_in_progress: bool,
    pub filter_mine: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            default_project_key: "".to_string(),
            filter_in_progress: true,
            filter_mine: true,
        }
    }
}

fn config_file_path() -> Result<PathBuf> {
    let mut path = app_root(AppDataType::UserConfig, &APP_INFO)?;
    path.push(CONFIG_FILE_NAME);
    return Ok(path);
}

pub fn load_config() -> Config {
    let path = match config_file_path() {
        Ok(p) => p,
        Err(_) => return Default::default(),
    };
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return Default::default(),
    };
    let reader = BufReader::new(file);

    // Read the JSON contents of the file as an instance of `Config`.
    let config: Config = match serde_json::from_reader(reader) {
        Ok(c) => c,
        Err(_) => Default::default(),
    };
    return config;
}

pub fn save_config(config: &Config) -> Result<()> {
    let file = File::create(config_file_path()?)?;
    serde_json::to_writer(file, &config)?;
    return Ok(());
}
