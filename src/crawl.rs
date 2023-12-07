use std::{
    ffi::{OsString},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

use anyhow::{Error, Result};
use indicatif::{MultiProgress, ProgressBar};
use tokio::{
    fs::read_dir,
    spawn,
    sync::{mpsc::UnboundedSender, Semaphore},
};

use crate::{cli::Validate, errors, terraform};

/// Crawl Arguments
pub struct Args {
    /// Progress Bar
    pub mp: MultiProgress,

    /// Error Channel
    pub chan: UnboundedSender<Error>,

    /// Current Path
    pub path: PathBuf,

    /// Crawl Semaphore to limit Concurrency
    pub crawl_semaphore: Arc<Semaphore>,

    /// Terraform Validation Semaphore to limit Concurrency
    pub validate_semaphore: Arc<Semaphore>,
}

struct FileExt(Option<OsString>);

enum FileType {
    DotFile,
    Directory(PathBuf),
    File(FileExt),
    Other,
}

async fn file_type(entry: &tokio::fs::DirEntry) -> Result<FileType> {
    if is_dot_file(entry) {
        return Ok(FileType::DotFile);
    }
    let file_type = entry.file_type().await?;
    if file_type.is_dir() {
        return Ok(FileType::Directory(entry.path()));
    }
    if file_type.is_file() {
        let path = entry.path();
        let ext = path.extension().map(|s| s.to_os_string());
        return Ok(FileType::File(FileExt(ext)));
    }
    Ok(FileType::Other)
}

fn is_dot_file(entry: &tokio::fs::DirEntry) -> bool {
    entry.file_name().to_string_lossy().starts_with('.')
}

fn validate(
    chan: UnboundedSender<Error>,
    state: &mut TfState,
    mp: &MultiProgress,
    path: PathBuf,
    semaphore: Arc<Semaphore>,
) {
    *state = TfState::Validated;

    let pb = mp.add(ProgressBar::new_spinner());
    pb.enable_steady_tick(Duration::from_millis(100));
    pb.set_message(format!("Validating {}", path.display()));

    spawn(async move {
        match terraform::run_validation(&semaphore, &path).await {
            Ok(()) => pb.finish_with_message(format!("✅ Finished Validating {}", path.display())),
            Err(err) => {
                pb.finish_with_message(format!(
                    "❌ Failed to Validate {}: {}",
                    path.display(),
                    err
                ));
                chan.send(err).expect("failed to send");
            }
        }
    });
}

#[derive(Copy, Clone)]
enum TfState {
    Validated,
    Unvalidated,
}

/// Recursively Crawl a directory using Tokio
async fn crawl(
    Args {
        mp,
        chan,
        path,
        crawl_semaphore,
        validate_semaphore,
    }: Args,
) {
    fn go(args: Args) {
        spawn(crawl(args));
    }

    let task = |chan: UnboundedSender<Error>| async move {
        let _permit = crawl_semaphore.acquire().await?;
        let mut entries = read_dir(&path).await?;
        let mut state = TfState::Unvalidated;
        while let Some(entry) = entries.next_entry().await? {
            match (file_type(&entry).await?, state) {
                (FileType::Directory(path), _) => go(Args {
                    chan: chan.clone(),
                    path,
                    mp: mp.clone(),
                    crawl_semaphore: crawl_semaphore.clone(),
                    validate_semaphore: validate_semaphore.clone(),
                }),
                (FileType::File(FileExt(Some(ext))), TfState::Unvalidated) if ext == "tf" => {
                    validate(
                        chan.clone(),
                        &mut state,
                        &mp,
                        path.clone(),
                        validate_semaphore.clone(),
                    );
                }
                (FileType::DotFile | FileType::File(_) | FileType::Other, _) => continue,
            }
        }
        Ok(())
    };

    match task(chan.clone()).await {
        Ok(()) => (),
        Err(err) => chan.send(err).expect("failed to send"),
    }
}

pub async fn start(
    Validate {
        max_concurrency_fs,
        max_concurrency_process,
    }: Validate,
) -> Result<(), Error> {
    let mp = MultiProgress::new();
    let crawl_semaphore = Arc::new(Semaphore::new(max_concurrency_fs));
    let validate_semaphore = Arc::new(Semaphore::new(max_concurrency_process));
    let path: PathBuf = ".".into();

    let (chan, mut recv) = tokio::sync::mpsc::unbounded_channel();

    spawn(crawl(Args {
        mp,
        chan,
        path,
        crawl_semaphore,
        validate_semaphore,
    }));

    let mut errors = vec![];
    while let Some(err) = recv.recv().await {
        errors.push(err)
    }

    errors::join(errors)
}
