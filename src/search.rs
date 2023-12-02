use std::{path::PathBuf, sync::Arc};

use anyhow::Error;
use tokio::sync::{
    mpsc::{unbounded_channel, UnboundedSender},
    Semaphore,
};

use crate::errors;

async fn read_dir_iter_impl(
    sender: UnboundedSender<Error>,
    path: PathBuf,
    fd_semaphore: Arc<Semaphore>,
) -> Result<(), Error> {
    println!("reading {}", path.display());

    let mut entries = tokio::fs::read_dir(path).await?;
    while let Some(entry) = entries.next_entry().await? {
        if entry.file_type().await?.is_dir() {
            spawn_task(sender.clone(), entry.path(), fd_semaphore.clone())
        }
    }

    Ok(())
}

async fn read_dir_iter_impl_limit(
    sender: UnboundedSender<Error>,
    path: PathBuf,
    fd_semaphore: Arc<Semaphore>,
) -> Result<(), Error> {
    let _permit = fd_semaphore.acquire().await?;
    read_dir_iter_impl(sender, path, fd_semaphore.clone()).await
}

// eliminate recursive type checking compiler error
fn spawn_task(sender: UnboundedSender<Error>, path: PathBuf, fd_semaphore: Arc<Semaphore>) {
    tokio::spawn(async move { read_dir_iter(sender, path, fd_semaphore).await });
}

// wrap `visit_dir` so we can handle errors
// not needed if we had `try` blocks
async fn read_dir_iter(
    sender: UnboundedSender<Error>,
    path: PathBuf,
    fd_semaphore: Arc<Semaphore>,
) {
    match read_dir_iter_impl_limit(sender.clone(), path, fd_semaphore).await {
        Ok(()) => (),
        Err(err) => sender.send(err).expect("failed to send"),
    }
}

pub async fn read_dir(max_concurrency: usize) -> Result<(), Error> {
    let fd_semaphore = Arc::new(Semaphore::new(max_concurrency));
    let path: PathBuf = ".".into();
    let (snd, mut recv) = unbounded_channel();

    spawn_task(snd, path, fd_semaphore);

    let mut errors = vec![];
    while let Some(err) = recv.recv().await {
        errors.push(err)
    }

    errors::join(errors)
}
