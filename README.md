# Jira TUI

Do you waste time figuring out the key for Jira tickets you're working on so that you can reference them in your branch names? Then this tool is for you!

## Install

Install Rust. [`rustup` is the easiest way to do that.](https://rustup.rs/)

Then, clone this repo and run: 
```
cargo install --path .
```

Published binaries coming soon!

Finally, you'll also need to export some env vars so that you can connect to Jira:

```
export JIRA_HOST=https://yourorg.atlassian.net
export JIRA_USER=iterion@gmail.com
export JIRA_PASS=<create a token>
```

It's recommended that you create an API token in order to use the API, you can create an API token [here](https://id.atlassian.com/manage-profile/security/api-tokens)

## Usage

Installation above places 2 binaries on your path named `git-branch-from-jira` and `jira`. This means that we can use this app directly from git!

```
git branch-from-jira
```

You might also want to add a git alias to make this quicker to type!

Or, if you prefer you can also just type:

```
jira
```
