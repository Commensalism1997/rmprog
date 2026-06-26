use std::{ffi::{OsString}, path::{Path, PathBuf}, pin::Pin, process::ExitCode, sync::{Arc, atomic::{AtomicBool, Ordering}}, time::Duration};
use futures::future::{join_all};
use tokio::{fs, io, task::{self, JoinHandle}};
use clap::Parser;
use indicatif::{MultiProgress, ProgressBar};

mod style;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Path of directory or file to remove
    path: Vec<PathBuf>,

    /// Print every deleted item in separate line
    #[arg(short, long)]
    verbose: bool
}

#[tokio::main]
async fn main() -> Result<ExitCode, Box<dyn std::error::Error>> {
    let cli: Cli = Cli::parse();
    let mpb = Arc::new(MultiProgress::new());
    let countpb = mpb.add(style::themed_progressbar(cli.path.len() as u64).with_message("Deleting..."));
    let mut handler: Vec<JoinHandle<Result<(), io::Error>>> = Vec::new();
    let success = Arc::new(AtomicBool::new(true));

    if cli.path.is_empty() {
        println!("Usage: rmprog [-v] <PATH>...");
        return Ok(ExitCode::SUCCESS)
    }

    for ipath in cli.path
    {
        if !fs::try_exists(&ipath).await? {
            success.store(false, Ordering::SeqCst);
            match ipath.file_name() {
                Some(n) => mpb.println(format!("{} does not exist, skipping", n.to_string_lossy()))?,
                None => mpb.println("Entry does not exist, skipping")?
            }
            countpb.set_length(countpb.length().expect("Total bar should have a length") - 1);
            continue;
        }
        let ipath = PathBuf::from(ipath.as_os_str().to_string_lossy().trim_end_matches('/'));
        if ipath.is_dir() && !fs::symlink_metadata(&ipath).await?.is_symlink() {
            let spinner = mpb.add(style::themed_spinner()).with_message("Analyzing...");
            spinner.enable_steady_tick(Duration::from_millis(100));
            let entry_count = count_dir(&ipath).await? + 1;
            let pb = mpb.add(style::themed_progressbar_no_msg(entry_count as u64));
            handler.push(task::spawn(full_remove_dir(ipath, pb, spinner, cli.verbose, mpb.clone())));
        }
        else {
            let pfname = ipath.file_name();
            let spinner = mpb.add(style::themed_spinner());
            handler.push(task::spawn(async_remove_file(ipath.clone(), cli.verbose, pfname.map(|f| f.to_owned()), spinner, mpb.clone())));
        }
    }
    let wrapped_handler = handler.into_iter().map(|f| async {
        match f.await.unwrap() {
            Ok(_v) => (),
            Err(e) => {
                success.store(false, Ordering::SeqCst);
                mpb.suspend(|| eprintln!("{}", e));
            }
        }
        countpb.inc(1);
    });
    join_all(wrapped_handler).await;
    countpb.finish_with_message("Done");

    Ok(if success.load(Ordering::SeqCst) { ExitCode::SUCCESS } else { ExitCode::FAILURE })
}

fn count_dir<'a>(path: &'a impl AsRef<Path>) -> Pin<Box<dyn Future<Output = io::Result<usize>> + 'a>> {
    Box::pin(async move {
        let mut reader = fs::read_dir(path).await?;
        let mut count: usize = 0;
        while let Some(e) = reader.next_entry().await? {
            if e.file_type().await?.is_dir() {
                let n = count_dir(&e.path()).await?;
                count += n + 1;
                continue;
            }
            count += 1;
        }
        Ok(count)
    })
}

async fn async_remove_file(p: impl AsRef<Path>, verbose: bool, pfname: Option<OsString>, spinner: ProgressBar, mpb: Arc<MultiProgress>) -> io::Result<()> {
    fs::remove_file(p).await?;
    if verbose {
        match pfname {
            Some(n) => mpb.println(format!("Removed file {}", n.to_string_lossy()))?,
            None => mpb.println("Removed file")?
        }
    }
    spinner.finish_and_clear();
    Ok(())
}

fn prev_remove_dir<'a>(path: &'a (impl AsRef<Path> + Sync), pb: &'a ProgressBar, spinner: &'a ProgressBar, verbose: bool, mpb: &'a MultiProgress) -> Pin<Box<dyn Future<Output = io::Result<()>> + Send + 'a>> {
    Box::pin(async move {
        let path = path.as_ref();
        let mut reader = fs::read_dir(path).await?;
        while let Some(e) = reader.next_entry().await? {
            if e.file_type().await?.is_dir() {
                prev_remove_dir(&e.path(), pb, spinner, verbose, mpb).await?;
                continue;
            }
            spinner.set_message(format!("Removing file {}...", e.file_name().to_string_lossy()));
            fs::remove_file(e.path()).await?;
            pb.inc(1);
            if verbose { mpb.println(format!("Removed file {}", e.file_name().to_string_lossy()))?; }
        }
        let pfname = path.file_name();
        match pfname {
            Some(n) => {
                spinner.set_message(format!("Removing directory {}...", n.to_string_lossy()));
                if verbose { mpb.println(format!("Removed directory {}", n.to_string_lossy()))?; }
            }
            None => spinner.set_message("Removing directory...")
        }
        fs::remove_dir_all(path).await?;
        pb.inc(1);
        Ok(())
    })
}

fn full_remove_dir<'a>(path: PathBuf, pb: ProgressBar, spinner: ProgressBar, verbose: bool, mpb: Arc<MultiProgress>) -> Pin<Box<dyn Future<Output = io::Result<()>> + Send + 'a>> {
    Box::pin(async move {
        let mut reader = fs::read_dir(&path).await?;
        while let Some(e) = reader.next_entry().await? {
            if e.file_type().await?.is_dir() {
                prev_remove_dir(&e.path(), &pb, &spinner, verbose, &mpb).await?;
                continue;
            }
            spinner.set_message(format!("Removing file {}...", e.file_name().to_string_lossy()));
            fs::remove_file(e.path()).await?;
            pb.inc(1);
            if verbose { mpb.println(format!("Removed file {}", e.file_name().to_string_lossy()))?; }
        }
        let pfname = path.file_name();
        match pfname {
            Some(n) => {
                spinner.set_message(format!("Removing directory {}...", n.to_string_lossy()));
                if verbose { mpb.println(format!("Removed directory {}", n.to_string_lossy()))?; }
            }
            None => spinner.set_message("Removing directory...")
        }
        fs::remove_dir_all(path).await?;
        pb.inc(1);
        pb.finish_and_clear();
        spinner.finish_with_message("Finished");
        spinner.finish_and_clear();
        Ok(())
    })
}
