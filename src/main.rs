extern crate app_dirs;
extern crate git2;
extern crate goji;
#[macro_use]
extern crate serde;
extern crate tokio;

mod config;
mod events;
mod git;
mod jira;
mod state;
mod ui;
mod utils;

use crate::{jira::JiraClient, state::State};
use anyhow::Result;
use app_dirs::AppInfo;
use tokio::sync::mpsc;

pub const APP_INFO: AppInfo = AppInfo {
    name: "jira-tui",
    author: "iterion",
};

#[tokio::main]
async fn main() -> Result<()> {
    // Create a Jira client
    let jira = JiraClient::new()?;

    let (event_tx, event_rx) = mpsc::unbounded_channel();
    events::subscribe_to_key_events(event_tx.clone());

    let state = State::new();
    let state_rx = state::updater(event_tx, event_rx, jira, state).await;

    if let Err(e) = ui::init_ui(state_rx).await {
        return Err(e);
    }

    Ok(())
}
