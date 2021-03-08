extern crate app_dirs;
extern crate git2;
extern crate goji;
#[macro_use]
extern crate serde;
extern crate tokio;

mod app;
mod config;
mod git;
mod jira;
mod ui;
mod utils;

use crate::{app::App, git::get_current_repo, jira::JiraClient};
use anyhow::Result;
use app_dirs::AppInfo;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event, EventStream},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::{future::FutureExt, StreamExt};
use std::io::{stdout, Write};
use tui::{backend::CrosstermBackend, Terminal};

pub const APP_INFO: AppInfo = AppInfo {
    name: "jira-tui",
    author: "iterion",
};

#[tokio::main]
async fn main() -> Result<()> {
    // Terminal initialization
    enable_raw_mode()?;

    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);

    let mut terminal = Terminal::new(backend)?;

    let jira = JiraClient::new()?;
    // Initialize TUI App
    let mut app = App::new(jira, get_current_repo()?).await?;

    let mut reader = EventStream::new();

    loop {
        let event = reader.next().fuse();
        terminal.draw(|f| ui::draw(f, &mut app))?;

        tokio::select! {
            maybe_event = event => {

                match maybe_event {
                    Some(Ok(event)) => {
                        if let Event::Key(input) = event {
                            if app.handle_input(input).await.is_err() {
                                break;
                            }
                        }
                    },
                    _ => break,
                }
            }
        }
    }

    // Do some cleanup
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
