use crate::config::Config;
use anyhow::{anyhow, Result};
use goji::{Credentials, Jira, SearchOptions};
use std::env;

pub struct JiraClient {
    jira: Jira,
}

impl JiraClient {
    pub fn new() -> Result<JiraClient> {
        if let (Ok(host), Ok(user), Ok(pass)) = (
            env::var("JIRA_HOST"),
            env::var("JIRA_USER"),
            env::var("JIRA_PASS"),
        ) {
            let jira = Jira::new(host, Credentials::Basic(user, pass))?;
            Ok(JiraClient { jira })
        } else {
            Err(anyhow!("Missing Jira Credentials"))
        }
    }

    pub fn current_issues(&self, config: &Config) -> Result<Vec<IssueSummary>> {
        // status=3 is "In Progress"
        let mut query_parts: Vec<String> = vec!();

        if config.filter_mine {
            query_parts.push("assignee=currentuser()".to_string());
        }

        if config.filter_in_progress {
            query_parts.push("status=3".to_string());
        } else {
            query_parts.push("status=\"Prioritised\"".to_string());
        }

        if config.default_project_key != "" {
            query_parts.push(format!("project = \"{}\"", config.default_project_key));
        }

        let query = query_parts.join(" AND ");

        let issues = match self
            .jira
            .search()
            .iter(query, &search_options_for_config(config))
        {
            Ok(results) => {
                results
                    .map(|issue| {
                        let summary = issue.summary().unwrap_or("No summary given".to_string());
                        // let assignee_name = match issue.assignee() {
                        //    Some(u) => u.display_name,
                        //    None => "Unassigned".to_string(),
                        // };
                        IssueSummary {
                            key: issue.key,
                            summary,
                            // assignee_name,
                        }
                    })
                    .collect()
            }
            Err(err) => panic!("{:#?}", err),
        };

        Ok(issues)
    }

    pub fn current_boards(&self, config: &Config) -> Result<Vec<BoardSummary>> {
        let boards = match self.jira.boards().iter(&search_options_for_config(config)) {
            Ok(results) => results
                .map(|board| {
                    BoardSummary {
                        key: board.id,
                        name: board.name,
                    }
                })
                .collect(),
            Err(err) => panic!("{:#?}", err),
        };

        Ok(boards)
    }
}

// #[derive(Serialize, Deserialize, Debug)]
pub struct IssueSummary {
    pub key: String,
    pub summary: String,
    // pub assignee_name: String,
}

fn search_options_for_config(config: &Config) -> SearchOptions {
    let mut options = SearchOptions::builder();
    options.max_results(100);
    if config.default_project_key != "" {
      options.project_key_or_id(&config.default_project_key);
    }
    options.build()
}

pub struct BoardSummary {
    pub key: u64,
    pub name: String,
}
