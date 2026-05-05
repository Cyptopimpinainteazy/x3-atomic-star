use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Deserialize)]
struct TaskRecord {
    id: String,
    title: String,
    feature: String,
    agent: String,
    permission_tier: String,
    status: String,
    approval_required: String,
    risk: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Starting x3-swarm-worker...");
    loop {
        let tasks = fetch_tasks().await?;
        if tasks.is_empty() {
            println!("No swarm tasks available, sleeping...");
        } else {
            println!("Found {} task(s).", tasks.len());
            for task in tasks {
                println!("- [{}] {} ({})", task.id, task.title, task.status);
            }
        }
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
}

async fn fetch_tasks() -> anyhow::Result<Vec<TaskRecord>> {
    let client = reqwest::Client::new();
    let response = client
        .get("http://127.0.0.1:8787/tasks")
        .send()
        .await?
        .error_for_status()?;
    let tasks = response.json::<Vec<TaskRecord>>().await?;
    Ok(tasks)
}
