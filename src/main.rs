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
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::Spans,
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

            f.render_stateful_widget(issues, chunks[0], &mut app.issues.state);
            f.render_stateful_widget(branches, chunks[1], &mut app.branches.state);
        })?;

        match events.next()? {
            Event::Input(input) => match input {
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
                                if let Some(key) = app.selected_issue_key() {
                                    match create_and_use_branch(&repo, key) {
                                        Ok(_) => break,
                                        Err(e) => println!("Error setting branch: {:?}", e),
                                    }
                                }
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
                    if app.issues_focused {
                        // Focus on first branch
                        app.branches.next();
                        app.issues_focused = false;
                    }
                }
                Key::Left => {
                    if app.issues_focused {
                        app.issues.unselect();
                        app.find_relevant_branches(&repo);
                    } else {
                        app.branches.unselect();
                        app.issues_focused = true;
                    }
                }
                Key::Down => {
                    if app.issues_focused {
                        app.issues.next();
                        app.find_relevant_branches(&repo);
                    } else {
                        app.branches.next();
                    }
                }
                Key::Up => {
                    if app.issues_focused {
                        app.issues.previous();
                        app.find_relevant_branches(&repo);
                    } else {
                        app.branches.previous();
                    }
                }
                _ => {}
            },
            Event::Tick => {}
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
            results
                .map(|issue| {
                    // println!("{:#?}", issue.status());
                    let summary = issue.summary().unwrap_or("No summary given".to_string());
                    IssueSummary {
                        key: issue.key,
                        summary: summary,
                    }
                })
                .collect()
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
    issues: StatefulList<IssueSummary>,
    branches: StatefulList<BranchSummary>,
    issues_focused: bool,
}

impl App {
    fn from_issues(issues: Vec<IssueSummary>) -> App {
        App {
            issues: StatefulList::with_items(issues),
            branches: StatefulList::new(),
            issues_focused: true,
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

    fn find_relevant_branches(&mut self, repo: &git2::Repository) -> Result<()> {
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
}

struct IssueSummary {
    key: String,
    summary: String,
}

struct BranchSummary {
    name: String,
}

// Done for Git side effects
fn create_and_use_branch(repo: &git2::Repository, branch_name: String) -> Result<()> {
    let master_branch = repo.refname_to_id("refs/heads/master")?;
    let master_commit = repo.find_commit(master_branch)?;
    if repo
        .find_branch(&branch_name, git2::BranchType::Local)
        .is_err()
    {
        let _ = repo.branch(&branch_name, &master_commit, false)?;
    }
    // let ref_name = reference.name()?.ok_or_else(|| anyhow!("Couldn't get new branch reference"))?;
    let refname = format!("refs/heads/{}", branch_name);
    repo.set_head(&refname)?;

    Ok(())
}

// Done for Git side effects
fn checkout_branch(repo: &git2::Repository, branch_name: String) -> Result<()> {
    let refname = format!("refs/heads/{}", branch_name);
    repo.set_head(&refname)?;

    Ok(())
}

fn matching_branches(repo: &git2::Repository, branch_name: String) -> Result<Vec<BranchSummary>> {
    let branches = repo.branches(Some(git2::BranchType::Local))?;
    Ok(branches
        .filter_map(|branch| {
            if let Ok((branch, branch_type)) = branch {
                let name = branch
                    .name()
                    .unwrap_or(None)
                    .unwrap_or("Invalid Branch")
                    .to_string();
                if name.starts_with(&branch_name) {
                    Some(BranchSummary { name: name })
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect())
}
