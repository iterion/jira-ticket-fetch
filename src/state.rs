use crate::{
    config::{load_config, save_config, Config},
    events::{Event, EventsRx, EventsTx},
    git::{
        checkout_branch,
        get_current_repo,
        create_and_use_branch,
        matching_branches,
        BranchSummary,
    },
    jira::{BoardSummary, IssueSummary, JiraClient},
    utils::StatefulList,
};
use anyhow::{bail, Result};
use crossterm::event::KeyCode;
use tokio::sync::mpsc;
// use git2::Repository;
use std::process::Command;

pub type StateRx = mpsc::Receiver<State>;

pub async fn updater(
    event_tx: EventsTx,
    mut event_rx: EventsRx,
    jira: JiraClient,
    mut state: State,
) -> StateRx {
    let (tx, rx) = mpsc::channel(20);

    // Prime the receiver with the initial state
    let _ = tx.send(state.clone()).await;

    fetch_tickets(event_tx.clone(), jira.clone(), state.clone()).await;

    tokio::spawn(async move {
        let tx = tx.clone();
        loop {
            if let Some(event_type) = event_rx.recv().await {
                match event_type {
                    Event::KeyEvent(code) => {
                        // TODO this is kind of weird, we use an error handle quitting, refactor
                        // with channels
                        if let Err(_) =
                            handle_input(&mut state, code, event_tx.clone(), jira.clone()).await
                        {
                            break;
                        }
                        let _ = tx.send(state.clone()).await;
                    }
                    Event::IssuesUpdated(issues) => {
                        state.issues = StatefulList::with_items(issues);
                        state.issues.next();
                        find_relevant_branches(event_tx.clone(), state.clone()).await;

                        let _ = tx.send(state.clone()).await;
                    }
                    Event::BranchesUpdated(branches) => {
                        state.branches.items = branches;
                        state.branches.items.push(BranchSummary {
                            name: "Create New".to_string(),
                        });

                        let _ = tx.send(state.clone()).await;
                    }
                    _ => {}
                }
            }
        }
    });

    rx
}

async fn fetch_tickets(event_tx: EventsTx, jira: JiraClient, state: State) {
    tokio::spawn(async move {
        if let Ok(issues) = jira.current_issues(&state.config).await {
            assert!(event_tx.send(Event::IssuesUpdated(issues)).is_ok())
        }
    });
}

async fn find_relevant_branches(event_tx: EventsTx, state: State) {
    if let Some(key) = state.selected_issue_key() {
        tokio::spawn(async move {
            if let Ok(repo) = get_current_repo() {
                if let Ok(branches) = matching_branches(&repo, key) {
                    assert!(event_tx.send(Event::BranchesUpdated(branches)).is_ok())
                }
            }
        });
    };
}

#[derive(Clone)]
pub enum InputMode {
    IssuesList,
    BoardsList,
    Editing,
    EditingDefaultProject,
}

#[derive(Clone)]
pub struct State {
    pub issues: StatefulList<IssueSummary>,
    pub boards: StatefulList<BoardSummary>,
    pub branches: StatefulList<BranchSummary>,
    pub config: Config,
    pub input_mode: InputMode,
    issues_focused: bool,
    input: String,
}

impl State {
    pub fn new() -> State {
        let config = load_config();
        // let boards = jira.current_boards(&config).await?;
        State {
            issues: StatefulList::new(),
            boards: StatefulList::new(),
            branches: StatefulList::new(),
            issues_focused: true,
            input_mode: InputMode::IssuesList,
            input: String::new(),
            config,
        }
    }

    fn selected_issue_key(&self) -> Option<String> {
        if let Some(i) = self.issues.state.selected() {
            if let Some(issue) = self.issues.items.get(i) {
                return Some(issue.key.clone());
            }
        }
        None
    }

    fn selected_issue_permalink(&self) -> Option<String> {
        match self.issues.state.selected() {
            Some(i) => Some(self.issues.items[i].permalink.clone()),
            None => None,
        }
    }

    fn open_selected_board(&self) {
        if let Some(i) = self.boards.state.selected() {
            let link = self.boards.items[i].permalink.clone();
            let _ = Command::new("open").arg(link).output();
        }
    }

    fn selected_branch_name(&self) -> Option<String> {
        match self.branches.state.selected() {
            Some(i) => Some(self.branches.items[i].name.clone()),
            None => None,
        }
    }

    pub fn raw_input_clone(&self) -> String {
        self.input.clone()
    }

    pub fn new_branch_name(&self) -> String {
        match self.selected_issue_key() {
            Some(key) => format!("{}-{}", key, self.input),
            None => "unhandled-error".to_string(),
        }
    }
}

async fn handle_input(
    state: &mut State,
    input: KeyCode,
    event_tx: EventsTx,
    jira: JiraClient,
) -> Result<()> {
    match state.input_mode {
        InputMode::IssuesList => match input {
            KeyCode::Char('b') => {
                state.input_mode = InputMode::BoardsList;
            }
            KeyCode::Char('c') => {
                state.input = state.config.default_project_key.clone();
                state.input_mode = InputMode::EditingDefaultProject;
            }
            KeyCode::Char('i') => {
                state.config.filter_in_progress = !state.config.filter_in_progress;
                let _ = save_config(&state.config);
                // TODO fix cloning
                fetch_tickets(event_tx, jira.clone(), state.clone()).await;
            }
            KeyCode::Char('m') => {
                state.config.filter_mine = !state.config.filter_mine;
                let _ = save_config(&state.config);
                // TODO fix cloning
                fetch_tickets(event_tx, jira.clone(), state.clone()).await;
            }
            KeyCode::Char('o') => {
                if let Some(link) = state.selected_issue_permalink() {
                    let _ = Command::new("open").arg(link).output();
                }
            }
            KeyCode::Char('q') => bail!("Just exiting early"),
            KeyCode::Char('r') => {
                // TODO fix cloning
                fetch_tickets(event_tx, jira.clone(), state.clone()).await;
            }
            KeyCode::Enter => {
                if state.issues_focused {
                    // Focus on first branch
                    state.branches.next();
                    state.issues_focused = false;
                } else if let Some(name) = state.selected_branch_name() {
                    if name == *"Create New" {
                        state.input_mode = InputMode::Editing;
                    } else {
                        let repo = get_current_repo().unwrap();
                        match checkout_branch(&repo, name) {
                            Ok(_) => bail!("Done!"),
                            Err(e) => println!("Error setting branch: {:?}", e),
                        }
                    }
                }
            }
            KeyCode::Right => {
                if state.issues_focused && state.selected_issue_key().is_some() {
                    // Focus on first branch
                    state.branches.next();
                    state.issues_focused = false;
                }
            }
            KeyCode::Left => {
                if state.issues_focused {
                    state.issues.unselect();
                    state.branches.items.clear();
                } else {
                    state.branches.unselect();
                    state.issues_focused = true;
                }
            }
            KeyCode::Down => {
                if state.issues_focused {
                    state.issues.next();
                    let _ = find_relevant_branches(event_tx.clone(), state.clone()).await;
                } else {
                    state.branches.next();
                }
            }
            KeyCode::Up => {
                if state.issues_focused {
                    state.issues.previous();
                    let _ = find_relevant_branches(event_tx.clone(), state.clone()).await;
                } else {
                    state.branches.previous();
                }
            }
            _ => {}
        },
        InputMode::BoardsList => match input {
            KeyCode::Esc => {
                state.input_mode = InputMode::IssuesList;
            }
            KeyCode::Enter => {}
            KeyCode::Down => {
                state.boards.next();
            }
            KeyCode::Up => {
                state.boards.previous();
            }
            KeyCode::Char('o') => {
                let _ = state.open_selected_board();
            }
            _ => {}
        },
        InputMode::Editing => match input {
            KeyCode::Enter =>  {
                if let Ok(repo) = get_current_repo() {
                    match create_and_use_branch(&repo, state.new_branch_name()) {
                        Ok(_) => bail!("Done!"),
                        Err(e) => println!("Error setting branch: {:?}", e),
                    }
                }
            },
            KeyCode::Char(c) => {
                state.input.push(c);
            }
            KeyCode::Backspace => {
                state.input.pop();
            }
            KeyCode::Esc => {
                state.input_mode = InputMode::IssuesList;
            }
            _ => {}
        },
        InputMode::EditingDefaultProject => match input {
            KeyCode::Enter => {
                state.config.default_project_key = state.input.to_string();
                match save_config(&state.config) {
                    Ok(_) => {
                        // self.reload_issues().await;
                        state.input_mode = InputMode::IssuesList;
                    }
                    Err(e) => {
                        state.input = e.to_string();
                    }
                }
            }
            KeyCode::Char(c) => {
                state.input.push(c);
            }
            KeyCode::Backspace => {
                state.input.pop();
            }
            KeyCode::Esc => {
                state.input_mode = InputMode::IssuesList;
            }
            _ => {}
        },
    }

    Ok(())
}
