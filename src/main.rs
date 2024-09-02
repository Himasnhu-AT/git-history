use git2::{DiffOptions, Repository};
use regex::Regex;
use serde::Serialize;
use std::fmt;
use std::fs;
use std::path::Path;

#[derive(Serialize)]
struct CommitDiff {
    file: String,
    diff: String,
}

#[derive(Serialize)]
struct CommitHistory {
    commit_id: String,
    author: String,
    commit_message: String,
    pl_and_issue_id: String,
    git_diff: Vec<CommitDiff>,
}

#[derive(Debug)]
enum CustomError {
    GitError(git2::Error),
    JsonError(serde_json::Error),
    IoError(std::io::Error),
}

impl fmt::Display for CustomError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CustomError::GitError(err) => write!(f, "Git error: {}", err),
            CustomError::JsonError(err) => write!(f, "JSON error: {}", err),
            CustomError::IoError(err) => write!(f, "IO error: {}", err),
        }
    }
}

impl From<std::io::Error> for CustomError {
    fn from(err: std::io::Error) -> CustomError {
        CustomError::IoError(err)
    }
}

impl From<git2::Error> for CustomError {
    fn from(err: git2::Error) -> CustomError {
        CustomError::GitError(err)
    }
}

impl From<serde_json::Error> for CustomError {
    fn from(err: serde_json::Error) -> CustomError {
        CustomError::JsonError(err)
    }
}

fn main() -> Result<(), CustomError> {
    // Replace with the path to your repository
    let repo_path = Path::new("../tos/");
    let repo = Repository::open(repo_path)?;

    // Get the HEAD commit
    let head = repo.head()?;
    let head_commit = head.peel_to_commit()?;
    let mut revwalk = repo.revwalk()?;
    revwalk.push(head_commit.id())?;

    let mut commit_history = Vec::new();

    for commit_id in revwalk {
        let commit = repo.find_commit(commit_id?)?;
        let author = commit.author();
        let message = commit.message().unwrap_or("");
        let commit_id = commit.id().to_string();

        // Extract Pull Request or Issue ID if present in the commit message
        let pl_and_issue_id = extract_pl_and_issue_id(message);

        // Get the diff for the commit
        let diff = get_commit_diff(&repo, &commit)?;

        // Create the commit history object
        let commit_entry = CommitHistory {
            commit_id,
            author: author.name().unwrap_or("").to_string(),
            commit_message: message.to_string(),
            pl_and_issue_id,
            git_diff: diff,
        };

        commit_history.push(commit_entry);
    }

    // Serialize the commit history to JSON
    let json_output = serde_json::to_string_pretty(&commit_history)?;
    println!("Completed");

    fs::write("commit_history.json", json_output)?;

    Ok(())
}

fn extract_pl_and_issue_id(commit_message: &str) -> String {
    // Assuming the PR or Issue ID is mentioned with a pattern like "PL#123" or "Issue #123"
    let pr_pattern = Regex::new(r"(PL|Issue)\s*#\d+").unwrap();
    pr_pattern
        .find(commit_message)
        .map_or("".to_string(), |m| m.as_str().to_string())
}

fn get_commit_diff(
    repo: &Repository,
    commit: &git2::Commit,
) -> Result<Vec<CommitDiff>, CustomError> {
    let mut diffs = Vec::new();
    let tree = commit.tree()?;

    // Get the parent commit, if available
    let parent = if commit.parents().len() > 0 {
        Some(commit.parent(0)?)
    } else {
        None
    };

    let parent_tree = parent.as_ref().map(|p| p.tree().unwrap());
    let mut diff_options = DiffOptions::new();
    let diff =
        repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut diff_options))?;

    let mut current_file: Option<String> = None;
    let mut accumulated_diff = String::new();

    diff.print(git2::DiffFormat::Patch, |delta, _hunk, line| {
        if let Some(file_path) = delta.new_file().path() {
            let file_path_str = file_path.to_string_lossy().to_string();

            if let Some(current) = &current_file {
                if current != &file_path_str {
                    // Push the previous file's accumulated diff
                    diffs.push(CommitDiff {
                        file: current.clone(),
                        diff: accumulated_diff.clone(),
                    });
                    // Reset the accumulated diff for the new file
                    accumulated_diff.clear();
                }
            }

            current_file = Some(file_path_str);
        }

        accumulated_diff.push_str(&String::from_utf8_lossy(line.content()).to_string());

        true
    })?;

    if let Some(current) = current_file {
        diffs.push(CommitDiff {
            file: current,
            diff: accumulated_diff,
        });
    }

    Ok(diffs)
}
