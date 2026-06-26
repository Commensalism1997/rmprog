use std::{fs, path::{Path, PathBuf}, process::ExitCode, time::Duration};
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

fn main() -> Result<ExitCode, Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let mpb = MultiProgress::new();
    let countpb = mpb.add(style::themed_progressbar(cli.path.len() as u64).with_message("Deleting..."));
    let mut success = true;

    if cli.path.is_empty() {
        println!("Usage: rmprog [-v] <PATH>...");
        return Ok(ExitCode::SUCCESS)
    }

    for ipath in cli.path
    {
        if !fs::exists(&ipath)? {
            success = false;
            match ipath.file_name() {
                Some(n) => mpb.println(format!("{} does not exist, skipping", n.to_string_lossy()))?,
                None => mpb.println("Entry does not exist, skipping")?
            }
            countpb.set_length(countpb.length().expect("Total bar should have a length") - 1);
            continue;
        }
        match ipath.file_name() {
            Some(n) => countpb.set_message(format!("Deleting {}...", n.to_string_lossy())),
            None => countpb.set_message("Deleting...")
        }
        let ipath = PathBuf::from(ipath.as_os_str().to_string_lossy().trim_end_matches('/'));
        if ipath.is_dir() && !fs::symlink_metadata(&ipath)?.is_symlink() {
            let spinner = mpb.add(style::themed_spinner()).with_message("Analyzing...");
            spinner.enable_steady_tick(Duration::from_millis(100));
            let entry_count = count_dir(&ipath)? + 1;
            let pb = mpb.add(style::themed_progressbar_no_msg(entry_count as u64));
            full_remove_dir(&ipath, &pb, &spinner, cli.verbose, &mpb)?;
            pb.finish_and_clear();
            spinner.finish_with_message("Finished");
            spinner.finish_and_clear();
        }
        else {
            let pfname = ipath.file_name();
            let spinner = mpb.add(style::themed_spinner());
            match pfname {
                Some(n) => spinner.set_message(format!("Deleting {}...", n.to_string_lossy())),
                None => spinner.set_message("Deleting file...")
            }
            fs::remove_file(&ipath)?;
            if cli.verbose {
                match pfname {
                    Some(n) => mpb.println(format!("Removed file {}", n.to_string_lossy()))?,
                    None => mpb.println("Removed file")?
                }
            }
            spinner.finish_and_clear();
        }
        countpb.inc(1);
    }
    countpb.finish_with_message("Done");

    Ok(if success { ExitCode::SUCCESS } else { ExitCode::FAILURE })
}

fn count_dir(path: impl AsRef<Path>) -> Result<usize, Box<dyn std::error::Error>> {
    let readdir = fs::read_dir(path)?;
    let mut count: usize = 0;
    for e in readdir {
        let e = e?;
        if e.file_type()?.is_dir() {
            let n = count_dir(e.path())?;
            count += n + 1;
            continue;
        }
        count += 1;
    }
    Ok(count)
}

fn full_remove_dir(path: &Path, pb: &ProgressBar, spinner: &ProgressBar, verbose: bool, mpb: &MultiProgress) -> Result<(), Box<dyn std::error::Error>> {
    let readdir = fs::read_dir(path)?;
    for e in readdir {
        let e = e?;
        if e.file_type()?.is_dir() {
            full_remove_dir(&e.path(), pb, spinner, verbose, mpb)?;
            continue;
        }
        spinner.set_message(format!("Removing file {}...", e.file_name().to_string_lossy()));
        fs::remove_file(e.path())?;
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
    fs::remove_dir_all(path)?;
    pb.inc(1);
    Ok(())
}
