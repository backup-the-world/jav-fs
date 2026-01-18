use clap::Parser;
use dashmap::DashMap;
use ignore::WalkBuilder;
use indicatif::{ProgressBar, ProgressStyle};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use jav_fs::{
    convert_smb_url_to_unc, extract_id_from_filename, extract_prefix_from_id, is_image_file,
    is_video_file,
};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(default_value = ".")]
    url: String,

    #[arg(short, long)]
    threads: Option<usize>,

    #[arg(long)]
    show_prefix: bool,

    #[arg(long)]
    show_duplicate: bool,
}

fn main() {
    let args = Args::parse();
    let scan_path = resolve_scan_path(&args.url);

    if let Err(e) = authenticate_smb_if_needed(&args.url) {
        eprintln!("{}", e);
        return;
    }

    if args.show_prefix {
        run_prefix_scan(&scan_path, args.threads);
    } else if args.show_duplicate {
        run_duplicate_scan(&scan_path, args.threads);
    } else {
        run_stats_scan(&scan_path, args.threads);
    }
}

fn run_stats_scan(scan_path: &str, threads: Option<usize>) {
    if scan_path.is_empty() {
        eprintln!("Invalid scan path");
        return;
    }

    let cnt = Arc::new(AtomicUsize::new(0));
    let video_cnt = Arc::new(AtomicUsize::new(0));
    let image_cnt = Arc::new(AtomicUsize::new(0));

    println!("Scanning path: {} (Stats Mode)", scan_path);
    let start = Instant::now();

    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] Scanned: {pos}")
            .unwrap()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
    );
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
        let image_cnt = image_cnt.clone();
        let pb = pb.clone();

        Box::new(move |result| {
            use ignore::WalkState;
            let entry = match result {
                Ok(entry) => entry,
                Err(_) => return WalkState::Continue,
            };

            let path = entry.path();
            if !path.is_file() {
                return WalkState::Continue;
            }

            cnt.fetch_add(1, Ordering::Relaxed);
            pb.inc(1);

            if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                if is_video_file(filename) {
                    video_cnt.fetch_add(1, Ordering::Relaxed);
                } else if is_image_file(filename) {
                    image_cnt.fetch_add(1, Ordering::Relaxed);
                }
            }

            WalkState::Continue
        })
    });

    pb.finish_and_clear();
    let duration = start.elapsed();

    println!("\nScan Result (completed in {:.2?}):", duration);
    println!("1. 文件总数量: {}", cnt.load(Ordering::Relaxed));
    println!("2. 视频文件数量: {}", video_cnt.load(Ordering::Relaxed));
    println!("3. 图片文件数量: {}", image_cnt.load(Ordering::Relaxed));
}

fn run_duplicate_scan(scan_path: &str, threads: Option<usize>) {
    if scan_path.is_empty() {
        eprintln!("Invalid scan path");
        return;
    }

    let cnt = Arc::new(AtomicUsize::new(0));
    let video_cnt = Arc::new(AtomicUsize::new(0));
    let files = Arc::new(DashMap::new());
    let files_failed = Arc::new(DashMap::new());
    let conflicts = Arc::new(Mutex::new(Vec::new()));

    println!("Scanning path: {} (Duplicate Mode)", scan_path);
    let start = Instant::now();

    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] Scanned: {pos} | Videos: {msg}",
        )
        .unwrap()
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
    );
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
                Err(_) => return WalkState::Continue,
            };

            let path = entry.path();
            if !path.is_file() {
                return WalkState::Continue;
            }

            cnt.fetch_add(1, Ordering::Relaxed);
            pb.inc(1);

            if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                if is_video_file(filename) {
                    let v_count = video_cnt.fetch_add(1, Ordering::Relaxed);
                    pb.set_message((v_count + 1).to_string());

                    let fullpath = path.to_string_lossy().to_string();
                    if let Some(id) = extract_id_from_filename(filename) {
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
                }
            }

            WalkState::Continue
        })
    });

    pb.finish_and_clear();
    let duration = start.elapsed();

    println!(
        "\nScan Complete (Duplicate Mode, completed in {:.2?}).",
        duration
    );
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

fn run_prefix_scan(scan_path: &str, threads: Option<usize>) {
    if scan_path.is_empty() {
        eprintln!("Invalid scan path");
        return;
    }

    let video_cnt = Arc::new(AtomicUsize::new(0));
    let prefixes_map = Arc::new(DashMap::new());

    println!("Scanning path: {} (Prefix Mode)", scan_path);
    let start = Instant::now();

    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] Scanned: {pos} | Prefixes: {msg}",
        )
        .unwrap()
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
    );
    pb.enable_steady_tick(Duration::from_millis(100));

    let mut builder = WalkBuilder::new(scan_path);
    builder.hidden(false);
    if let Some(t) = threads {
        builder.threads(t);
    }

    let walker = builder.build_parallel();

    walker.run(|| {
        let video_cnt = video_cnt.clone();
        let prefixes_map = prefixes_map.clone();
        let pb = pb.clone();

        Box::new(move |result| {
            use ignore::WalkState;
            let entry = match result {
                Ok(entry) => entry,
                Err(_) => return WalkState::Continue,
            };

            let path = entry.path();
            if !path.is_file() {
                return WalkState::Continue;
            }

            pb.inc(1);

            if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                if is_video_file(filename) {
                    video_cnt.fetch_add(1, Ordering::Relaxed);

                    if let Some(id) = extract_id_from_filename(filename) {
                        if let Some(prefix) = extract_prefix_from_id(&id) {
                            if prefixes_map.insert(prefix.to_uppercase(), ()).is_none() {
                                pb.set_message(prefixes_map.len().to_string());
                            }
                        }
                    }
                }
            }

            WalkState::Continue
        })
    });

    pb.finish_and_clear();
    let duration = start.elapsed();

    let mut prefix_list: Vec<String> = prefixes_map.iter().map(|kv| kv.key().clone()).collect();
    prefix_list.sort();
    if !prefix_list.is_empty() {
        println!("\nUnique Prefixes (completed in {:.2?}):", duration);
        println!("{}", prefix_list.join(", "));
    } else {
        println!("\nNo prefixes found (completed in {:.2?}).", duration);
    }
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
