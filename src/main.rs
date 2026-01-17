use clap::Parser;
use dashmap::DashMap;
use ignore::WalkBuilder;
use indicatif::{ProgressBar, ProgressStyle};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use jav_fs::{convert_smb_url_to_unc, extract_id_from_filename, is_video_file};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(default_value = ".")]
    url: String,

    #[arg(short, long)]
    threads: Option<usize>,
}

fn main() {
    let args = Args::parse();
    let scan_path = resolve_scan_path(&args.url);

    if let Err(e) = authenticate_smb_if_needed(&args.url) {
        eprintln!("{}", e);
        return;
    }

    run_scan(&scan_path, args.threads);
}

fn resolve_scan_path(url: &str) -> String {
    if url.starts_with("smb://") {
        match convert_smb_url_to_unc(url) {
            Ok(unc_path) => {
                println!("Converted SMB URL to UNC Path: {}", unc_path);
                unc_path
            }
            Err(e) => {
                eprintln!("Error parsing URL: {}", e);
                String::new()
            }
        }
    } else {
        url.to_string()
    }
}

fn authenticate_smb_if_needed(url: &str) -> Result<(), String> {
    if !url.starts_with("smb://") {
        return Ok(());
    }

    use std::process::Command;
    use url::Url;

    let parsed_url = Url::parse(url).map_err(|e| format!("Failed to parse URL: {}", e))?;
    let host = parsed_url.host_str().ok_or("Missing host in URL")?;

    let username = parsed_url.username();
    let password = parsed_url.password().unwrap_or("");

    if !username.is_empty() {
        let auth_target = format!("\\\\{}\\IPC$", host);
        println!(
            "Attempting SMB authentication for {} as {}...",
            host, username
        );

        let mut cmd = Command::new("net");
        cmd.args([
            "use",
            &auth_target,
            password,
            &format!("/USER:{}", username),
        ]);

        match cmd.output() {
            Ok(output) => {
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    if stderr.contains("1219") {
                        println!(
                            "Existing session found (Error 1219), proceeding with current session."
                        );
                    } else {
                        return Err(format!(
                            "Warning: SMB Authentication failed: {}",
                            stderr.trim()
                        ));
                    }
                } else {
                    println!("SMB Authentication successful.");
                }
            }
            Err(e) => {
                return Err(format!("Failed to execute 'net use' command: {}", e));
            }
        }
    }

    Ok(())
}

fn run_scan(scan_path: &str, threads: Option<usize>) {
    if scan_path.is_empty() {
        eprintln!("Invalid scan path");
        return;
    }

    let cnt = Arc::new(AtomicUsize::new(0));
    let video_cnt = Arc::new(AtomicUsize::new(0));

    let files = Arc::new(DashMap::new());
    let files_failed = Arc::new(DashMap::new());
    let conflicts = Arc::new(Mutex::new(Vec::new()));

    println!("Scanning path: {} (Parallel)", scan_path);

    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] Scanned: {pos} | Videos: {msg}",
        )
        .unwrap()
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
    );
    pb.set_message("0");
    pb.enable_steady_tick(Duration::from_millis(100));

    let mut builder = WalkBuilder::new(scan_path);
    builder.hidden(false);

    if let Some(t) = threads {
        builder.threads(t);
    }

    let walker = builder.build_parallel();

    walker.run(|| {
        let cnt = cnt.clone();
        let video_cnt = video_cnt.clone();
        let files = files.clone();
        let files_failed = files_failed.clone();
        let conflicts = conflicts.clone();
        let pb = pb.clone();

        Box::new(move |result| {
            use ignore::WalkState;

            let entry = match result {
                Ok(entry) => entry,
                Err(err) => {
                    pb.suspend(|| eprintln!("ERROR processing entry: {}", err));
                    return WalkState::Continue;
                }
            };

            let path = entry.path();
            if !path.is_file() {
                return WalkState::Continue;
            }

            cnt.fetch_add(1, Ordering::Relaxed);
            pb.inc(1);

            let filename = match path.file_name() {
                Some(name) => name.to_string_lossy(),
                None => return WalkState::Continue,
            };

            if !is_video_file(&filename) {
                return WalkState::Continue;
            }

            let v_count = video_cnt.fetch_add(1, Ordering::Relaxed);
            pb.set_message((v_count + 1).to_string());

            let fullpath = path.to_string_lossy().to_string();

            if let Some(id) = extract_id_from_filename(&filename) {
                match files.entry(id) {
                    dashmap::mapref::entry::Entry::Vacant(e) => {
                        e.insert(fullpath);
                    }
                    dashmap::mapref::entry::Entry::Occupied(_) => {
                        let mut c = conflicts.lock().unwrap();
                        c.push(fullpath);
                    }
                }
            } else {
                files_failed.insert(filename.to_string(), fullpath);
            }

            WalkState::Continue
        })
    });

    pb.finish_with_message(video_cnt.load(Ordering::Relaxed).to_string());

    println!("\nScan Complete.");
    println!("scanned files: {}", cnt.load(Ordering::Relaxed));
    println!("videos files: {}", video_cnt.load(Ordering::Relaxed));
    println!("actual videos: {}", files.len());
    println!("failed videos: {}", files_failed.len());

    let conflicts_vec = conflicts.lock().unwrap();
    if !conflicts_vec.is_empty() {
        println!("\nConflicts (Duplicate IDs found):");
        for entry in conflicts_vec.iter() {
            println!("{}", entry);
        }
    }
}
