use std::{path::Path, process::Output};

use anyhow::{anyhow, Result};
use tokio::{process::Command, sync::Semaphore};

async fn rate_limit(semaphore: &Semaphore, mut cmd: Command) -> Result<Output> {
    let _permit = semaphore.acquire().await?;
    Ok(cmd.output().await?)
}

async fn init(semaphore: &Semaphore, path: &Path) -> Result<()> {
    let mut cmd = Command::new("terraform");

    cmd.current_dir(path)
        .arg("init")
        .arg("-input=false")
        .arg("-backend=false");

    let output = rate_limit(semaphore, cmd).await?;

    if output.status.success() {
        Ok(())
    } else {
        let error = String::from_utf8(output.stderr)?;
        Err(anyhow!("terraform init failed: {}", error))
    }
}

async fn validate(semaphore: &Semaphore, path: &Path) -> Result<()> {
    let mut cmd = Command::new("terraform");

    cmd.current_dir(path).arg("validate");

    let output = rate_limit(semaphore, cmd).await?;

    if output.status.success() {
        Ok(())
    } else {
        let error = String::from_utf8(output.stderr)?;
        Err(anyhow!("terraform validate failed: {}", error))
    }
}

pub async fn run_validation(semaphore: &Semaphore, path: &Path) -> Result<()> {
    //init(semaphore, path).await?;
    //validate(semaphore, path).await
    Ok(())
}
