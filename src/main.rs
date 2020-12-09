extern crate app_dirs;
extern crate git2;
extern crate goji;

mod git;
mod jira;
mod utils;

const APP_INFO: AppInfo = AppInfo {
    name: "jira-tui",
    author: "iterion",
};

use crate::git::{
    checkout_branch, create_and_use_branch, get_current_repo, matching_branches, BranchSummary,
};
use crate::jira::{get_current_issues, IssueSummary};
use crate::utils::{
    event::Event,
    StatefulList,
};
use anyhow::{bail, Result};
use app_dirs::*;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event as CEvent, KeyCode, KeyEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use git2::Repository;
use std::{
    error::Error,
    io::{stdout, Write},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
use tui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Spans,
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Terminal,
};

enum InputMode {
    IssuesList,
    Editing,
}

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
        terminal.draw(|f| {
            let size = f.size();

            let help_drawer = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(10), Constraint::Length(2)])
                .split(size);

            let help = Paragraph::new("Arrows: Navigate - Enter: Select")
                .style(Style::default().fg(Color::White))
                .block(Block::default().borders(Borders::NONE));
            f.render_widget(help, help_drawer[1]);

            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
                .split(help_drawer[0]);

            let issues: Vec<ListItem> = app
                .issues
                .items
                .iter()
                .map(|i| {
                    let line_content = format!("{}: {}", i.key, i.summary);
                    let lines = vec![Spans::from(line_content)];
                    ListItem::new(lines).style(Style::default().fg(Color::Black).bg(Color::White))
                })
                .collect();
            let issues = List::new(issues)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("In Progress Jira Issues"),
                )
                .highlight_style(
                    Style::default()
                        .bg(Color::LightGreen)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol(">> ");
            f.render_stateful_widget(issues, chunks[0], &mut app.issues.state);

            let branches: Vec<ListItem> = app
                .branches
                .items
                .iter()
                .map(|i| {
                    let line_content = format!("{}", i.name);
                    let lines = vec![Spans::from(line_content)];
                    ListItem::new(lines).style(Style::default().fg(Color::Black).bg(Color::White))
                })
                .collect();
            let branches = List::new(branches)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Existing Branches"),
                )
                .highlight_style(
                    Style::default()
                        .bg(Color::LightGreen)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol(">> ");

            f.render_stateful_widget(branches, chunks[1], &mut app.branches.state);

            match app.input_mode {
                InputMode::Editing => {
                    let area = centered_rect(60, 20, size);
                    let input = Paragraph::new(app.new_branch_name().clone())
                        .style(Style::default().fg(Color::Yellow))
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title("Enter new branch name"),
                        );
                    f.render_widget(Clear, area);
                    f.render_widget(input, area);
                    f.set_cursor(
                        // Put cursor past the end of the input text
                        area.x + app.new_branch_name().len() as u16 + 1,
                        // Move one line down, from the border to the input line
                        area.y + 1,
                    )
                }
                _ => (),
            }
        })?;

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
            },
            Event::Tick => {}
        }
    }
    Ok(())
}

struct App {
    issues: StatefulList<IssueSummary>,
    branches: StatefulList<BranchSummary>,
    issues_focused: bool,
    input_mode: InputMode,
    input: String,
    repo: Repository,
}

impl App {
    fn from_issues_and_repo(issues: Vec<IssueSummary>, repo: Repository) -> App {
        App {
            issues: StatefulList::with_items(issues),
            branches: StatefulList::new(),
            issues_focused: true,
            input_mode: InputMode::IssuesList,
            input: String::new(),
            repo: repo,
        }
    }

    fn selected_issue_key(&self) -> Option<String> {
        match self.issues.state.selected() {
            Some(i) => Some(self.issues.items[i].key.clone()),
            None => None,
        }
    }

    fn selected_branch_name(&self) -> Option<String> {
        match self.branches.state.selected() {
            Some(i) => Some(self.branches.items[i].name.clone()),
            None => None,
        }
    }

    fn find_relevant_branches(&mut self) -> Result<()> {
        // Clear out current listed branches
        self.branches.items.clear();
        if let Some(key) = self.selected_issue_key() {
            let branches = matching_branches(&self.repo, key)?;
            self.branches.items = branches;
            self.branches.items.push(BranchSummary {
                name: "Create New".to_string(),
            });
        };

        Ok(())
    }

    fn new_branch_name(&self) -> String {
        match self.selected_issue_key() {
            Some(key) => format!("{}-{}", key, self.input),
            None => "unhandled-error".to_string(),
        }
    }

    // fn get_jira_cache_file(&self) -> Result<PathBuf> {
    //     let mut dir = app_dir(AppDataType::UserCache, &APP_INFO, "cache/")?;
    //     dir.push("jira.json");
    //     Ok(dir)
    // }
    fn handle_input(&mut self, input: KeyEvent) -> Result<()> {
        match self.input_mode {
            InputMode::IssuesList => {
                match input.code {
                    KeyCode::Char('q') => bail!("Just exiting early"),
                    KeyCode::Enter => {
                        if self.issues_focused {
                            // Focus on first branch
                            self.branches.next();
                            self.issues_focused = false;
                        } else {
                            if let Some(name) = self.selected_branch_name() {
                                // TODO more efficient comparison
                                if name == "Create New".to_string() {
                                    self.input_mode = InputMode::Editing;
                                } else {
                                    match checkout_branch(&self.repo, name) {
                                        Ok(_) => bail!("Done!"),
                                        Err(e) => println!("Error setting branch: {:?}", e),
                                    }
                                }
                            }
                        }
                    }
                    KeyCode::Right => {
                        if self.issues_focused && self.selected_issue_key().is_some() {
                            // Focus on first branch
                            self.branches.next();
                            self.issues_focused = false;
                        }
                    }
                    KeyCode::Left => {
                        if self.issues_focused {
                            self.issues.unselect();
                            // TODO add an error view and surface them if they occur
                            let _ = self.find_relevant_branches();
                        } else {
                            self.branches.unselect();
                            self.issues_focused = true;
                        }
                    }
                    KeyCode::Down => {
                        if self.issues_focused {
                            self.issues.next();
                            // TODO add an error view and surface them if they occur
                            let _ = self.find_relevant_branches();
                        } else {
                            self.branches.next();
                        }
                    }
                    KeyCode::Up => {
                        if self.issues_focused {
                            self.issues.previous();
                            // TODO add an error view and surface them if they occur
                            let _ = self.find_relevant_branches();
                        } else {
                            self.branches.previous();
                        }
                    }
                    _ => {}
                }
            }
            InputMode::Editing => match input.code {
                KeyCode::Enter => match create_and_use_branch(&self.repo, self.new_branch_name()) {
                    Ok(_) => bail!("Done!"),
                    Err(e) => println!("Error setting branch: {:?}", e),
                },
                KeyCode::Char(c) => {
                    self.input.push(c);
                }
                KeyCode::Backspace => {
                    self.input.pop();
                }
                KeyCode::Esc => {
                    self.input_mode = InputMode::IssuesList;
                    // events.enable_exit_key();
                }
                _ => {}
            },
        }
        Ok(())
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ]
            .as_ref(),
        )
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ]
            .as_ref(),
        )
        .split(popup_layout[1])[1]
}
