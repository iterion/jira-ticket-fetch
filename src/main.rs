extern crate app_dirs;
extern crate git2;
extern crate goji;

mod git;
mod jira;
mod ui;
mod utils;
mod app;

const APP_INFO: AppInfo = AppInfo {
    name: "jira-tui",
    author: "iterion",
};

use crate::{
    git::get_current_repo,
    jira::get_current_issues,
    utils::event::Event,
    app::App,
};
use anyhow::{Result};
use app_dirs::*;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event as CEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

use std::{
    io::{stdout, Write},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
use tui::{
    backend::CrosstermBackend,
    Terminal,
};

fn main() -> Result<()> {
    // Terminal initialization
    enable_raw_mode()?;

    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);

    let mut terminal = Terminal::new(backend)?;

    // Setup input handling
    let (tx, rx) = mpsc::channel();

    let tick_rate = Duration::from_millis(250);
    thread::spawn(move || {
        let mut last_tick = Instant::now();
        loop {
            // poll for tick rate duration, if no events, sent tick event.
            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));
            if event::poll(timeout).unwrap() {
                if let CEvent::Key(key) = event::read().unwrap() {
                    tx.send(Event::Input(key)).unwrap();
                }
            }
            if last_tick.elapsed() >= tick_rate {
                tx.send(Event::Tick).unwrap();
                last_tick = Instant::now();
            }
        }
    });

    // Initialize TUI App
    let mut app = App::from_issues_and_repo(get_current_issues()?, get_current_repo()?);

    // Select the first if it exists
    app.issues.next();
    // TODO add an error view and surface them if they occur
    let _ = app.find_relevant_branches();

    loop {
        terminal.draw(|f| ui::draw(f, &mut app))?;

        match rx.recv()? {
            Event::Input(input) => {
                if app.handle_input(input).is_err() {
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
            Event::Tick => {}
        }
    }
    Ok(())
}
