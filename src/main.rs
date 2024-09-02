use git2::{DiffOptions, Repository};
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, Server, StatusCode};
use regex::Regex;
use serde::Serialize;
use std::convert::Infallible;
use std::env;
use std::fmt;
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::tempdir;

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

#[allow(dead_code)]
#[derive(Debug)]
enum CustomError {
    GitError(git2::Error),
    JsonError(serde_json::Error),
    IoError(std::io::Error),
    MissingFieldError(String),
}

impl fmt::Display for CustomError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CustomError::GitError(err) => write!(f, "Git error: {}", err),
            CustomError::JsonError(err) => write!(f, "JSON error: {}", err),
            CustomError::IoError(err) => write!(f, "IO error: {}", err),
            CustomError::MissingFieldError(field) => write!(f, "Missing field in JSON: {}", field),
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

#[tokio::main]
async fn main() -> Result<(), CustomError> {
    // Capture command-line arguments
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("\n\n         Git History \nUsage: cargo run --release [COMMAND] <args> \nIndex Code: cargo run --release index <path_to_repo> \nRun Server: cargo run --release server\n");
        return Ok(());
    }

    match args[1].as_str() {
        "index" => {
            if args.len() != 3 {
                eprintln!("Usage: cargo run --release index <path_to_repo>");
                return Ok(());
            }
            let repo_path = &args[2];
            let json_data = git_index(repo_path)?;
            fs::write(
                Path::new(".").join("commit_history.json"),
                json_data,
            )
            .map_err(|e| {
                eprintln!("Failed to write commit history to file: {}", e);
                CustomError::IoError(e)
            })?;
            println!("Commit history written {}", Path::new(".").join("commit_history.json").display());
            Ok(())
        }
        "server" => run_server().await,
        _ => {
            eprintln!("\n\n         Git History \nUsage: cargo run --release [COMMAND] <args> \nIndex Code: cargo run --release index <path_to_repo> \nRun Server: cargo run --release server\n");
            Ok(())
        }
    }
}


async fn run_server() -> Result<(), CustomError> {
    let make_svc =
        make_service_fn(|_conn| async { Ok::<_, Infallible>(service_fn(handle_request)) });

    let addr = ([0, 0, 0, 0], 8080).into();
    let server = Server::bind(&addr).serve(make_svc);

    println!("Server running on http://127.0.0.1:8080");

    server
        .await
        .map_err(|e| CustomError::IoError(std::io::Error::new(std::io::ErrorKind::Other, e)))
}

async fn handle_request(req: Request<Body>) -> Result<Response<Body>, Infallible> {
    let response = match (req.method(), req.uri().path()) {
        (&Method::POST, "/git_history") => {
            let full_body = hyper::body::to_bytes(req.into_body()).await.unwrap();
            let parsed_body: serde_json::Value = serde_json::from_slice(&full_body).unwrap();
            if let Some(repo_url) = parsed_body["repo_url"].as_str() {
                match process_git_repo(repo_url).await {
                    Ok(json_response) => Response::new(Body::from(json_response)),
                    Err(e) => {
                        let error_message = format!("Error: {}", e);
                        Response::builder()
                            .status(StatusCode::INTERNAL_SERVER_ERROR)
                            .body(Body::from(error_message))
                            .unwrap()
                    }
                }
            } else {
                Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Body::from("Missing field: repo_url"))
                    .unwrap()
            }
        }
        _ => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("Not Found"))
            .unwrap(),
    };

    Ok(response)
}

async fn process_git_repo(repo_url: &str) -> Result<String, CustomError> {
    let temp_dir = tempdir().map_err(|e| {
        eprintln!("Failed to create temporary directory: {}", e);
        CustomError::IoError(e)
    })?;
    let clone_dir = temp_dir.path().join("repo");

    let status = Command::new("git")
        .arg("clone")
        .arg(format!("https://{}", repo_url))
        .arg(&clone_dir)
        .status()
        .map_err(|e| {
            eprintln!("Failed to run git command: {}", e);
            CustomError::IoError(e)
        })?;

    if !status.success() {
        return Err(CustomError::IoError(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Failed to clone repository: {}", repo_url),
        )));
    }

    let json_data = git_index(clone_dir.to_str().unwrap()).map_err(|e| {
        eprintln!("Failed to index git repository: {}", e);
        e
    })?;

    // Delete the temporary directory
    temp_dir.close().map_err(|e| {
        eprintln!("Failed to delete temporary directory: {}", e);
        CustomError::IoError(e)
    })?;

    Ok(json_data)
}

fn git_index(repo_path: &str) -> Result<String, CustomError> {
    let repo = Repository::open(Path::new(repo_path))?;

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
    let json_output = serde_json::to_string_pretty(&commit_history).map_err(|e| {
        eprintln!("Failed to serialize commit history to JSON: {}", e);
        CustomError::JsonError(e)
    })?;
    println!("Completed");

    Ok(json_output)
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
