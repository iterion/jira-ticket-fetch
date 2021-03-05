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

    // Setup input handling
    // let (tx, rx) = mpsc::channel();

    // let tick_rate = Duration::from_millis(250);
    // thread::spawn(move || {
    //     let mut last_tick = Instant::now();
    //     loop {
    //         // poll for tick rate duration, if no events, sent tick event.
    //         let timeout = tick_rate
    //             .checked_sub(last_tick.elapsed())
    //             .unwrap_or_else(|| Duration::from_secs(0));
    //         if event::poll(timeout).unwrap() {
    //             if let CEvent::Key(key) = event::read().unwrap() {
    //                 tx.send(Event::Input(key)).unwrap();
    //             }
    //         }
    //         if last_tick.elapsed() >= tick_rate {
    //             tx.send(Event::Tick).unwrap();
    //             last_tick = Instant::now();
    //         }
    //     }
    // });

    let jira = JiraClient::new()?;
    // Initialize TUI App
    let mut app = App::new(jira, get_current_repo()?).await?;

    // Select the first if it exists
    app.issues.next();
    // TODO add an error view and surface them if they occur
    let _ = app.find_relevant_branches();

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
                                disable_raw_mode()?;
                                execute!(
                                    terminal.backend_mut(),
                                    LeaveAlternateScreen,
                                    DisableMouseCapture
                                )?;
                                terminal.show_cursor()?;
                                break;
                            }
                        }
                    },
                    _ => break,
                }
            }
        }
    }
    Ok(())
}
