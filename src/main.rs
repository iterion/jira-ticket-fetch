extern crate git2;
extern crate goji;

mod git;
mod jira;
mod utils;

use crate::git::{
    checkout_branch, create_and_use_branch, get_current_repo, matching_branches, BranchSummary,
};
use crate::jira::{get_current_issues, IssueSummary};
use crate::utils::{
    event::{Event, Events},
    StatefulList,
};
use anyhow::Result;
use git2::Repository;
use std::io;
use termion::{event::Key, input::MouseTerminal, raw::IntoRawMode, screen::AlternateScreen};
use tui::{
    backend::TermionBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::Spans,
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Terminal,
};

enum InputMode {
    IssuesList,
    Editing,
}

fn main() -> Result<()> {
    // Terminal initialization
    let stdout = io::stdout().into_raw_mode()?;
    let stdout = MouseTerminal::from(stdout);
    let stdout = AlternateScreen::from(stdout);
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let repo = get_current_repo()?;

    let issues = get_current_issues()?;

    // Initialize TUI Events
    let mut events = Events::new();
    // Initialize TUI App
    let mut app = App::from_issues(issues);

    // Select the first if it exists
    app.issues.next();
    // TODO add an error view and surface them if they occur
    let _ = app.find_relevant_branches(&repo);

    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
                .split(f.size());

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

            match app.input_mode {
                InputMode::IssuesList => {
                    let branches: Vec<ListItem> = app
                        .branches
                        .items
                        .iter()
                        .map(|i| {
                            let line_content = format!("{}", i.name);
                            let lines = vec![Spans::from(line_content)];
                            ListItem::new(lines)
                                .style(Style::default().fg(Color::Black).bg(Color::White))
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
                }
                InputMode::Editing => {
                    let input = Paragraph::new(app.new_branch_name().clone())
                        .style(Style::default().fg(Color::Yellow))
                        .block(Block::default().borders(Borders::ALL).title("Input"));
                    f.render_widget(input, chunks[1]);
                    f.set_cursor(
                        // Put cursor past the end of the input text
                        chunks[1].x + app.new_branch_name().len() as u16 + 1,
                        // Move one line down, from the border to the input line
                        chunks[1].y + 1,
                    )
                }
            }
        })?;

        match events.next()? {
            Event::Input(input) => {
                match app.input_mode {
                    InputMode::IssuesList => {
                        match input {
                            Key::Char('q') => {
                                break;
                            }
                            Key::Char('\n') => {
                                if app.issues_focused {
                                    // Focus on first branch
                                    app.branches.next();
                                    app.issues_focused = false;
                                } else {
                                    if let Some(name) = app.selected_branch_name() {
                                        // TODO more efficient comparison
                                        if name == "Create New".to_string() {
                                            app.input_mode = InputMode::Editing;
                                            events.disable_exit_key();
                                        } else {
                                            match checkout_branch(&repo, name) {
                                                Ok(_) => break,
                                                Err(e) => println!("Error setting branch: {:?}", e),
                                            }
                                        }
                                    }
                                }
                            }
                            Key::Right => {
                                if app.issues_focused && app.selected_issue_key().is_some() {
                                    // Focus on first branch
                                    app.branches.next();
                                    app.issues_focused = false;
                                }
                            }
                            Key::Left => {
                                if app.issues_focused {
                                    app.issues.unselect();
                                    // TODO add an error view and surface them if they occur
                                    let _ = app.find_relevant_branches(&repo);
                                } else {
                                    app.branches.unselect();
                                    app.issues_focused = true;
                                }
                            }
                            Key::Down => {
                                if app.issues_focused {
                                    app.issues.next();
                                    // TODO add an error view and surface them if they occur
                                    let _ = app.find_relevant_branches(&repo);
                                } else {
                                    app.branches.next();
                                }
                            }
                            Key::Up => {
                                if app.issues_focused {
                                    app.issues.previous();
                                    // TODO add an error view and surface them if they occur
                                    let _ = app.find_relevant_branches(&repo);
                                } else {
                                    app.branches.previous();
                                }
                            }
                            _ => {}
                        }
                    }
                    InputMode::Editing => match input {
                        Key::Char('\n') => {
                            match create_and_use_branch(&repo, app.new_branch_name()) {
                                Ok(_) => break,
                                Err(e) => println!("Error setting branch: {:?}", e),
                            }
                        }
                        Key::Char(c) => {
                            app.input.push(c);
                        }
                        Key::Backspace => {
                            app.input.pop();
                        }
                        Key::Esc => {
                            app.input_mode = InputMode::IssuesList;
                            events.enable_exit_key();
                        }
                        _ => {}
                    },
                }
            }
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
}

impl App {
    fn from_issues(issues: Vec<IssueSummary>) -> App {
        App {
            issues: StatefulList::with_items(issues),
            branches: StatefulList::new(),
            issues_focused: true,
            input_mode: InputMode::IssuesList,
            input: String::new(),
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

    fn find_relevant_branches(&mut self, repo: &Repository) -> Result<()> {
        // Clear out current listed branches
        self.branches.items.clear();
        if let Some(key) = self.selected_issue_key() {
            let branches = matching_branches(repo, key)?;
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
}
