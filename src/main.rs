use clap::Parser;
use dashmap::DashMap;
use ignore::WalkBuilder;
use indicatif::{ProgressBar, ProgressStyle};
use regex::Regex;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use url::Url;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to scan (e.g. //192.168.3.11/jav/media/ or smb://user:pass@host/share)
    #[arg(default_value = ".")]
    url: String,

    /// Number of threads to use (default: automatic)
    #[arg(short, long)]
    threads: Option<usize>,
}

fn main() {
    let args = Args::parse();
    let mut scan_path = args.url.clone();

    // SMB URL Handling
    if scan_path.starts_with("smb://") {
        match Url::parse(&args.url) {
            Ok(url) => {
                if let Some(host) = url.host_str() {
                    // 1. Construct UNC Path
                    let mut path_parts = Vec::new();
                    if let Some(segments) = url.path_segments() {
                        for segment in segments {
                            // Filter empty segments (e.g. trailing slash)
                            if !segment.is_empty() {
                                path_parts.push(segment);
                            }
                        }
                    }

                    let unc_suffix = path_parts.join("\\");
                    let unc_path = if unc_suffix.is_empty() {
                        format!("\\\\{}", host)
                    } else {
                        format!("\\\\{}\\{}", host, unc_suffix)
                    };

                    println!("Converted SMB URL to UNC Path: {}", unc_path);

                    // 2. Handle Authentication
                    let username = url.username();
                    let password = url.password().unwrap_or("");

                    if !username.is_empty() {
                        // Authenticate against IPC$ share to establish session
                        let auth_target = format!("\\\\{}\\IPC$", host);
                        println!("Attempting SMB authentication for {} as {}...", host, username);

                        let mut cmd = Command::new("net");
                        cmd.args(["use", &auth_target, password, &format!("/USER:{}", username)]);

                        // Suppress output unless error, to avoid leaking info or cluttering
                        match cmd.output() {
                            Ok(output) => {
                                if !output.status.success() {
                                    let stderr = String::from_utf8_lossy(&output.stderr);
                                    // Error 1219: Multiple connections to a server... by the same user
                                    if stderr.contains("1219") {
                                        println!("Existing session found (Error 1219), proceeding with current session.");
                                    } else {
                                        eprintln!("Warning: SMB Authentication failed: {}", stderr.trim());
                                    }
                                } else {
                                    println!("SMB Authentication successful.");
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to execute 'net use' command: {}", e);
                            }
                        }
                    }

                    scan_path = unc_path;
                }
            }
            Err(e) => {
                eprintln!("Error parsing URL: {}", e);
                return;
            }
        }
    }

    let re_video = Regex::new(r".*\.(?i)(mp4|mkv|wmv)").unwrap();
    let re_file = Regex::new(r"[[:alpha:]]+-\d+|[[:alpha:]]+\d+").unwrap();

    let cnt = Arc::new(AtomicUsize::new(0));
    let video_cnt = Arc::new(AtomicUsize::new(0));

    // Map: extracted_id -> full_path
    let files = Arc::new(DashMap::new());
    // Map: filename -> full_path (for failed extractions)
    let files_failed = Arc::new(DashMap::new());
    // List of conflicting paths
    let conflicts = Arc::new(Mutex::new(Vec::new()));

    println!("Scanning path: {} (Parallel)", scan_path);

    // Initialize Progress Bar
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] Scanned: {pos} | Videos: {msg}")
            .unwrap()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
    );
    pb.set_message("0");
    pb.enable_steady_tick(Duration::from_millis(100));

    let mut builder = WalkBuilder::new(&scan_path);
    // Configure builder to traverse hidden files to match standard `WalkDir` behavior mostly
    // but still respect .ignore/.gitignore files if present.
    builder.hidden(false);

    if let Some(t) = args.threads {
        builder.threads(t);
    }

    let walker = builder.build_parallel();

    walker.run(|| {
        let re_video = re_video.clone();
        let re_file = re_file.clone();
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
                    // Suspend PB to print error cleanly
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

            if !re_video.is_match(&filename) {
                return WalkState::Continue;
            }

            let v_count = video_cnt.fetch_add(1, Ordering::Relaxed);
            pb.set_message((v_count + 1).to_string());

            let fullpath = path.to_string_lossy().to_string();

            if let Some(mat) = re_file.find(&filename) {
                let match_result = mat.as_str().to_owned();

                // Atomic check-and-insert
                match files.entry(match_result) {
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