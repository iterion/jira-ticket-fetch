extern crate git2;
extern crate goji;

mod utils;

use crate::utils::{
    event::{Event, Events},
    StatefulList,
};
use anyhow::{anyhow, Context, Result};
use git2::Repository;
use goji::{Credentials, Jira};
use std::{env, io};
use termion::{event::Key, input::MouseTerminal, raw::IntoRawMode, screen::AlternateScreen};
use tui::{
    backend::TermionBackend,
    layout::{Constraint, Corner, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, List, ListItem},
    Terminal,
};

fn main() -> Result<()> {
    // Terminal initialization
    let stdout = io::stdout().into_raw_mode()?;
    let stdout = MouseTerminal::from(stdout);
    let stdout = AlternateScreen::from(stdout);
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let repo = get_current_repo()?;

    let master_branch = repo.refname_to_id("refs/heads/master")?;
    let master_commit = repo.find_commit(master_branch)?;
    // println!("{:?}", repo.state());
    repo.branch("test-branch", &master_commit, false)?;
    let issues = get_current_issues()?;

    // Initialize TUI Events
    let events = Events::new();
    // Initialize TUI App
    let mut app = App::from_issues(issues);

    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
                .split(f.size());

            let items: Vec<ListItem> = app
                .items
                .items
                .iter()
                .map(|i| {
                    let lines = vec![Spans::from(i.key.clone())];
                    ListItem::new(lines).style(Style::default().fg(Color::Black).bg(Color::White))
                })
                .collect();
            let items = List::new(items)
                .block(Block::default().borders(Borders::ALL).title("In Progress Jira Issues"))
                .highlight_style(
                    Style::default()
                        .bg(Color::LightGreen)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol(">> ");
            f.render_stateful_widget(items, chunks[0], &mut app.items.state);
        })?;

        match events.next()? {
            Event::Input(input) => match input {
                Key::Char('q') => {
                    break;
                }
                Key::Left => {
                    app.items.unselect();
                }
                Key::Down => {
                    app.items.next();
                }
                Key::Up => {
                    app.items.previous();
                }
                _ => {}
            },
            Event::Tick => {
                // app.advance();
            }
        }
    }
    Ok(())
}

fn get_current_repo() -> Result<Repository> {
    let path = env::current_dir().context("Couldn't get the current directory")?;
    // println!("{:?}", path);
    Ok(Repository::discover(path).context("Couldn't find a git repo at the current directory")?)
}

fn get_current_issues() -> Result<Vec<IssueSummary>> {
    let jira = get_jira_client()?;

    let query = env::args()
        .nth(1)
        .unwrap_or("assignee=currentuser() AND status=3".to_owned());
    // status=3 is "In Progress"

    let issues = match jira.search().iter(query, &Default::default()) {
        Ok(results) => {
            results.map(|issue| {
                // println!("{:#?}", issue.key);
                // println!("{:#?}", issue.status());
                IssueSummary{key: issue.key, status_name: "In Progress".to_string()}
            }).collect()
        }
        Err(err) => panic!("{:#?}", err),
    };

    Ok(issues)
}

fn get_jira_client() -> Result<Jira> {
    if let (Ok(host), Ok(user), Ok(pass)) = (
        env::var("JIRA_HOST"),
        env::var("JIRA_USER"),
        env::var("JIRA_PASS"),
    ) {
        Ok(Jira::new(host, Credentials::Basic(user, pass))?)
    } else {
        Err(anyhow!("Missing Jira Credentials"))
    }
}

struct App {
    items: StatefulList<IssueSummary>,
}

impl App {
    fn from_issues(issues: Vec<IssueSummary>) -> App {
        App {
            items: StatefulList::with_items(issues)
            // items: StatefulList::with_items(vec![
            //     IssueSummary{key: "Test".to_string(), status_name: "In Progress".to_string()},
            // ]),
        }
    }
}

struct IssueSummary {
    key: String,
    status_name: String,
}
