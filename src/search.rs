use std::{cell::OnceCell, ops::Deref, path::PathBuf, sync::Arc};

use anyhow::{Error, Result};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use tokio::sync::{
    mpsc::{unbounded_channel, UnboundedSender},
    Semaphore,
};

use crate::errors;
use crate::{cli::Validate, terraform};

#[derive(Clone)]
struct CrawlSemaphore(Arc<Semaphore>);

#[derive(Clone)]
struct ProcessSemaphore(Arc<Semaphore>);

impl Deref for CrawlSemaphore {
    type Target = Arc<Semaphore>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Deref for ProcessSemaphore {
    type Target = Arc<Semaphore>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone)]
struct ErrorChannel(UnboundedSender<Error>);

impl Deref for ErrorChannel {
    type Target = UnboundedSender<Error>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

async fn crawl_directory(
    mp: MultiProgress,
    chan: ErrorChannel,
    path: PathBuf,
    crawl_semaphore: CrawlSemaphore,
    process_semaphore: ProcessSemaphore,
) -> Result<()> {
    let _permit = crawl_semaphore.acquire().await?;
    let mut entries = tokio::fs::read_dir(&path).await?;
    let cell = OnceCell::new();

    while let Some(entry) = entries.next_entry().await? {
        if entry.file_type().await?.is_dir() {
            spawn_crawl_directory_task(
                mp.clone(),
                chan.clone(),
                entry.path(),
                crawl_semaphore.clone(),
                process_semaphore.clone(),
            );
            continue;
        }
        let path = entry.path();
        let Some(ext) = path.extension() else {
            continue;
        };
        if ext != "tf" {
            continue;
        }
        cell.get_or_init(|| spawn_validation_task(chan.clone(), process_semaphore.clone(), path));
    }

    Ok(())
}

fn spawn_validation_task(chan: ErrorChannel, semaphore: ProcessSemaphore, path: PathBuf) {
    tokio::spawn(async move {
        match terraform::run_validation(&semaphore, &path).await {
            Ok(()) => (),
            Err(err) => chan
                .send(err)
                .expect("spawn_validation_task failed to send"),
        }
    });
}

fn spawn_crawl_directory_task(
    mp: MultiProgress,
    chan: ErrorChannel,
    path: PathBuf,
    crawl_semaphore: CrawlSemaphore,
    process_semaphore: ProcessSemaphore,
) {
    tokio::spawn(async move {
        let pb = mp.add(ProgressBar::new_spinner());
        pb.enable_steady_tick(tokio::time::Duration::from_millis(100));
        pb.set_message(format!("Crawling {}", path.display()));

        match crawl_directory(
            mp.clone(),
            chan.clone(),
            path.clone(),
            crawl_semaphore,
            process_semaphore,
        )
        .await
        {
            Ok(()) => {
                pb.finish_with_message(format!("✅ Finished Crawling {}", path.display()));
                mp.remove(&pb);
            }
            Err(err) => {
                pb.finish_with_message(format!("❌ Failed Crawl {}: {}", path.display(), err));

                chan.send(err)
                    .expect("spawn_crawl_directory_task failed to send")
            }
        }
    });
}

pub async fn read_dir(
    Validate {
        max_concurrency_fs,
        max_concurrency_process,
    }: Validate,
) -> Result<(), Error> {
    let bar = MultiProgress::new();
    let crawl_semaphore = CrawlSemaphore(Arc::new(Semaphore::new(max_concurrency_fs)));
    let process_semaphore = ProcessSemaphore(Arc::new(Semaphore::new(max_concurrency_process)));
    let path: PathBuf = ".".into();
    let (snd, mut recv) = unbounded_channel();

    spawn_crawl_directory_task(
        bar,
        ErrorChannel(snd),
        path,
        crawl_semaphore,
        process_semaphore,
    );

    let mut errors = vec![];
    while let Some(err) = recv.recv().await {
        errors.push(err)
    }

    errors::join(errors)
}
