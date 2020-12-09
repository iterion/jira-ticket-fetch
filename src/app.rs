use crate::{
    git::{checkout_branch, create_and_use_branch, matching_branches, BranchSummary},
    jira::IssueSummary,
    utils::StatefulList,
};
use anyhow::{bail, Result};
use crossterm::event::{KeyCode, KeyEvent};
use git2::Repository;

pub enum InputMode {
    IssuesList,
    Editing,
}

pub struct App {
    pub issues: StatefulList<IssueSummary>,
    pub branches: StatefulList<BranchSummary>,
    issues_focused: bool,
    pub input_mode: InputMode,
    input: String,
    repo: Repository,
}

impl App {
    pub fn from_issues_and_repo(issues: Vec<IssueSummary>, repo: Repository) -> App {
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

    pub fn find_relevant_branches(&mut self) -> Result<()> {
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

    pub fn new_branch_name(&self) -> String {
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
    pub fn handle_input(&mut self, input: KeyEvent) -> Result<()> {
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
