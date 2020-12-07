use anyhow::{anyhow, Result};
use goji::{Credentials, Jira};
use std::env;

// #[derive(Serialize, Deserialize, Debug)]
pub struct IssueSummary {
    pub key: String,
    pub summary: String,
}

pub fn get_current_issues() -> Result<Vec<IssueSummary>> {
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

pub fn get_jira_client() -> Result<Jira> {
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
