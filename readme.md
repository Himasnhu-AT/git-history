# Git-History

`Git-History` is a tool that generates a JSON representation of the Git history for a repository. You can use it either as a command-line tool or as a server providing an API.

## Table of Contents

- [Usage](#usage)
  - [Run Locally](#run-locally)
  - [Run as a Server](#run-as-a-server)
- [API](#api)
- [Example](#example)
- [Running in Docker](#running-in-docker)
- [JSON Structure](#json-structure)
- [License](#license)

## Usage

### Run Locally

Generate a JSON file containing Git history:

```bash
cargo run --release index <path_to_git_repo>
```
This command will create a JSON file in the current directory and print it to the terminal.

### Run as a Server

Start a server to provide Git history via an API:

```bash
cargo run --release server
```
The server will be available at `http://localhost:8080`.

## API

- **Endpoint:** `POST /git_history`
  - **URL:** `http://localhost:8080/git_history`
  - **Request Body:**
    ```json
    {
      "repo": "<path_to_git_repo>"
    }
    ```
  - **Response:** A JSON object containing the Git history for the specified repository.

## Example

Request Git history using `curl`:

```bash
curl -X POST -H "Content-Type: application/json" -d '{"repo": "github.com/himanshu-at/git-history"}' http://localhost:8080/git_history
```

## Running in Docker

### Build and Run Locally

Build and run the Docker image:

```bash
docker compose up -d
```

### Use Pre-built Docker Image

Alternatively, use the pre-built Docker image from Docker Hub:

Create a `docker-compose.yml` file:

```dockercompose
services:
  backend:
    image: himanshu806/git-history:0.1.0
    # build:
    #   context: .
    #   dockerfile: dockerfile
    ports:
      - 8080:8080
    command: cargo run --release server
```

## JSON Structure

The JSON response includes an array of commit objects with the following structure:

```json
[
  {
    "commit_id": "commit_hash",
    "author": "author_name",
    "message": "commit_message",
    "pl_and_issue_id": "pull_request_and_issue_id",
    "files": [
      {
        "file": "file_name",
        "diff": "diff_content"
      }
    ]
  }
]
```

## License

This project is licensed under the [MIT License](https://opensource.org/licenses/MIT).
