use anyhow::Context;
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
    required_commands: Option<Vec<String>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    println!("Starting x3-swarm-worker...");

    loop {
        let tasks = fetch_tasks(&client).await?;
        let pending: Vec<_> = tasks
            .into_iter()
            .filter(|task| task.status == "Pending")
            .collect();

        if pending.is_empty() {
            println!("No pending swarm tasks available, sleeping...");
        } else {
            println!("Found {} pending task(s).", pending.len());
            for task in pending {
                if task.approval_required == "manual" {
                    println!("Skipping manual approval task {}: {}", task.id, task.title);
                    continue;
                }
                process_task(&client, task).await?;
            }
        }

        tokio::time::sleep(Duration::from_secs(10)).await;
    }
}

async fn fetch_tasks(client: &reqwest::Client) -> anyhow::Result<Vec<TaskRecord>> {
    let response = client
        .get("http://127.0.0.1:8787/tasks")
        .send()
        .await
        .context("failed to request tasks from swarm API")?
        .error_for_status()?;
    let tasks = response.json::<Vec<TaskRecord>>().await?;
    Ok(tasks)
}

async fn process_task(client: &reqwest::Client, task: TaskRecord) -> anyhow::Result<()> {
    println!("Starting task {}: {}", task.id, task.title);
    post_task_action(client, &task.id, "start").await?;

    let commands = task.required_commands.unwrap_or_default();
    if !commands.is_empty() {
        println!("Simulating commands: {:?}", commands);
    }
    tokio::time::sleep(Duration::from_secs(3)).await;

    let action = if task.risk.to_lowercase() == "high" { "fail" } else { "complete" };
    let completed = post_task_action(client, &task.id, action).await?;
    println!("Task {} completed with status {}", completed.id, completed.status);
    Ok(())
}

async fn post_task_action(client: &reqwest::Client, task_id: &str, action: &str) -> anyhow::Result<TaskRecord> {
    let url = format!("http://127.0.0.1:8787/tasks/{}/{}", task_id, action);
    let response = client
        .post(&url)
        .send()
        .await
        .with_context(|| format!("failed to post {} for task {}", action, task_id))?
        .error_for_status()?;

    let task = response.json::<TaskRecord>().await?;
    Ok(task)
}
