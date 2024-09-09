from dotenv.main import load_dotenv
import requests
import json
import os
import logging
from concurrent.futures import ThreadPoolExecutor, as_completed

# Setup logging
logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')
logger = logging.getLogger(__name__)

# Add GitHub token to bypass rate limit
load_dotenv()
GITHUB_TOKEN = os.getenv("GITHUB_TOKEN")
if not GITHUB_TOKEN:
    raise ValueError("GitHub token not found in .env file")

# Function to fetch all pull requests
def fetch_all_prs(owner, repo):
    url = f"https://api.github.com/repos/{owner}/{repo}/pulls?state=closed"
    headers = {"Authorization": f"token {GITHUB_TOKEN}"}
    response = requests.get(url, headers=headers)
    response.raise_for_status()
    return response.json()

# Function to fetch a specific pull request
def fetch_pr(owner, repo, pr_number):
    url = f"https://api.github.com/repos/{owner}/{repo}/pulls/{pr_number}"
    headers = {"Authorization": f"token {GITHUB_TOKEN}"}
    response = requests.get(url, headers=headers)
    response.raise_for_status()
    return response.json()

# Function to fetch commits related to a pull request
def fetch_commits(owner, repo, pr_number):
    url = f"https://api.github.com/repos/{owner}/{repo}/pulls/{pr_number}/commits"
    headers = {"Authorization": f"token {GITHUB_TOKEN}"}
    response = requests.get(url, headers=headers)
    response.raise_for_status()
    return response.json()

# Function to fetch commit details from SHA URL
def getDetailsFromSha(url):
    headers = {"Authorization": f"token {GITHUB_TOKEN}"}
    response = requests.get(url, headers=headers)
    response.raise_for_status()
    return response.json()

# Function to get the content of the file
def get_file_content(owner, repo, file_sha):
    url = f"https://api.github.com/repos/{owner}/{repo}/git/blobs/{file_sha}"
    headers = {"Authorization": f"token {GITHUB_TOKEN}", "Accept": "application/vnd.github.v3.raw"}
    response = requests.get(url, headers=headers)
    response.raise_for_status()
    return response.text

# Function to generate JSON from PR data
def generate_json(pr_title, pr_description, commits, owner, repo):
    result = {
        "prompt": f"issue: {pr_title} | description: {pr_description}",
        "completion": []
    }

    for commit in commits:
        commit_details = getDetailsFromSha(commit["url"])
        for file in commit_details["files"]:
            file_data = {
                "file": file["filename"],
                "git_diff": file.get("patch", "")
            }
            # Fetch the content of the file if available
            if "sha" in file:
                try:
                    file_content = get_file_content(owner, repo, file["sha"])
                    file_data["file_content"] = file_content
                except Exception as e:
                    logger.warning(f"Could not fetch content for file {file['filename']}: {e}")
            result["completion"].append(file_data)

    return result

# Worker function to fetch PR details and generate JSON
def process_pr(pr, owner, repo):
    pr_number = pr["number"]
    logger.info(f"Processing PR #{pr_number}")

    pr_details = fetch_pr(owner, repo, pr_number)

    # Get PR title and description
    pr_title = pr_details["title"]
    pr_description = pr_details.get("body", "")

    # Fetch commits related to the PR
    commits = fetch_commits(owner, repo, pr_number)

    # Generate the JSON structure for each PR
    pr_json_data = generate_json(pr_title, pr_description, commits, owner, repo)

    return pr_json_data

if __name__ == "__main__":
    owner = "facebook"
    repo = "react"

    logger.info("Starting...")
    all_prs = fetch_all_prs(owner, repo)

    logger.info(f"Fetched total PRs: {len(all_prs)}")

    merged_data = []

    # Set up the ThreadPoolExecutor for parallel processing
    with ThreadPoolExecutor(max_workers=5) as executor:
        # Submit all PRs to the thread pool for processing
        futures = [executor.submit(process_pr, pr, owner, repo) for pr in all_prs]

        # Collect the results as they complete
        for future in as_completed(futures):
            try:
                pr_json_data = future.result()
                merged_data.append(pr_json_data)
            except Exception as exc:
                logger.error(f"Error processing PR: {exc}")

    print("divided data for training and testing")
    total_count = len(merged_data)
    train_size = int(total_count * 0.9)

    training_data = merged_data[:train_size]
    testing_data = merged_data[train_size:]

    # Save training data
    with open("training_data.json", "w") as train_file:
        json.dump(training_data, train_file, indent=4)
    logger.info("Training data saved to training_data.json")

    # Save testing data
    with open("testing_data.json", "w") as test_file:
        json.dump(testing_data, test_file, indent=4)
    logger.info("Testing data saved to testing_data.json")

    logger.info("All PR data processing completed")
