use clap::{Parser, Subcommand};
use dashmap::DashMap;
use ignore::WalkBuilder;
use indicatif::{ProgressBar, ProgressStyle};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use jav_fs::{
    apply_planned_videos, convert_smb_url_to_unc, extract_id_from_filename, extract_prefix_from_id,
    is_distinct_video_part, is_image_file, is_video_file, load_organize_options,
    run_organize_dry_run_with_progress, HttpImageDownloader, OrganizeCliOptions, OrganizeCliPaths,
    OrganizeProgress, StdFileMover,
};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,

    #[arg(default_value = ".")]
    url: String,

    #[arg(short, long)]
    threads: Option<usize>,

    #[arg(long)]
    show_prefix: bool,

    #[arg(long)]
    show_duplicate: bool,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// 预览或执行 JAV 媒体库入库整理
    Organize(OrganizeArgs),
}

#[derive(Parser, Debug)]
struct OrganizeArgs {
    #[arg(long)]
    source: Option<String>,

    #[arg(long)]
    target: Option<String>,

    #[arg(long)]
    database: Option<String>,

    #[arg(long)]
    apply: bool,

    #[arg(long)]
    fail_fast: bool,

    #[arg(long)]
    exclude: Vec<String>,
}

fn main() {
    let args = Args::parse();

    if let Some(command) = args.command {
        match command {
            Command::Organize(organize_args) => run_organize(organize_args),
        }
        return;
    }

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

fn run_organize(args: OrganizeArgs) {
    let cli = OrganizeCliOptions {
        paths: OrganizeCliPaths {
            source: args.source,
            target: args.target,
            database: args.database,
        },
        apply: args.apply,
        fail_fast: args.fail_fast,
        exclude: args.exclude,
    };

    match load_organize_options(cli, "jav-fs.toml") {
        Ok(options) => {
            if options.apply {
                println!("Organize apply (artwork, NFO generation, and video move).");
            } else {
                println!("Organize dry-run (no files will be written, downloaded, or moved).");
            }
            println!("Source: {}", options.source.display());
            println!("Target: {}", options.target.display());
            println!("Database: {}", options.database.display());
            if !options.exclude.is_empty() {
                println!("Exclude: {}", options.exclude.join(", "));
            }

            eprintln!("[organize] scanning source...");
            match run_organize_dry_run_with_progress(&options, |progress| match progress {
                OrganizeProgress::Scan(progress) => {
                    eprintln!(
                        "[organize] scanned entries: {} | video candidates: {} | unrecognized: {}",
                        progress.scanned_entries,
                        progress.video_candidates,
                        progress.unrecognized_videos
                    );
                }
                OrganizeProgress::MetadataLookup { processed, total } => {
                    eprintln!("[organize] metadata lookup: {}/{}", processed, total);
                }
                OrganizeProgress::Planning => {
                    eprintln!("[organize] planning target paths...");
                }
            }) {
                Ok(report) => {
                    eprintln!("[organize] scan/database/plan complete.");
                    println!("Candidates: {}", report.candidate_count);
                    println!("Recognized with metadata: {}", report.candidates.len());
                    if options.apply {
                        println!("Already organized: {}", 0);
                    } else {
                        println!("Will organize: {}", report.planned_videos.len());
                    }
                    for planned in &report.planned_videos {
                        println!(
                            "  Plan: {} -> {} (NFO: {})",
                            planned.source_path.display(),
                            planned.target_video_path.display(),
                            planned.nfo_path.display()
                        );
                    }
                    println!(
                        "Already exists skipped: {}",
                        report.target_name_conflicts.len()
                    );
                    println!("Batch conflicts: {}", report.batch_conflicts.len());
                    println!(
                        "Target name conflicts: {}",
                        report.target_name_conflicts.len()
                    );
                    println!("Missing metadata: {}", report.missing_metadata.len());
                    println!("Missing actress info: {}", report.missing_actresses.len());
                    println!(
                        "Missing release dates: {}",
                        report.missing_release_dates.len()
                    );
                    println!("Empty titles: {}", report.empty_titles.len());
                    println!("Path warnings: {}", report.path_warnings.len());
                    println!("Unrecognized videos: {}", report.unrecognized_videos.len());
                    for path in report.unrecognized_videos {
                        println!("  Unrecognized: {}", path.display());
                    }
                    if options.apply {
                        let apply_report = apply_planned_videos(
                            &report.planned_videos,
                            &HttpImageDownloader,
                            &StdFileMover,
                            options.fail_fast,
                        );
                        println!("Already organized: {}", apply_report.moved_videos.len());
                        println!("Moved videos: {}", apply_report.moved_videos.len());
                        println!("NFO failures: {}", apply_report.nfo_failures.len());
                        println!("Move failures: {}", apply_report.move_failures.len());
                        println!(
                            "Source delete failures: {}",
                            apply_report.source_delete_failures.len()
                        );
                        println!("Artwork warnings: {}", apply_report.artwork_warnings.len());
                        for warning in apply_report.artwork_warnings {
                            println!("  Artwork warning: {}", warning);
                        }
                    }
                    for candidate in report.missing_metadata {
                        println!(
                            "  Missing metadata: {} ({})",
                            candidate.product_id,
                            candidate.path.display()
                        );
                    }
                }
                Err(e) => {
                    eprintln!("{}", e);
                    std::process::exit(2);
                }
            }
        }
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(2);
        }
    }
}

fn run_stats_scan(scan_path: &str, threads: Option<usize>) {
    if scan_path.is_empty() {
        eprintln!("Invalid scan path");
        return;
    }

    let cnt = Arc::new(AtomicUsize::new(0));
    let video_cnt = Arc::new(AtomicUsize::new(0));
    let video_size_sum = Arc::new(AtomicUsize::new(0));
    let image_cnt = Arc::new(AtomicUsize::new(0));
    let image_size_sum = Arc::new(AtomicUsize::new(0));

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
        let video_size_sum = video_size_sum.clone();
        let image_cnt = image_cnt.clone();
        let image_size_sum = image_size_sum.clone();
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
                    if let Ok(metadata) = entry.metadata() {
                        video_size_sum.fetch_add(metadata.len() as usize, Ordering::Relaxed);
                    }
                } else if is_image_file(filename) {
                    image_cnt.fetch_add(1, Ordering::Relaxed);
                    if let Ok(metadata) = entry.metadata() {
                        image_size_sum.fetch_add(metadata.len() as usize, Ordering::Relaxed);
                    }
                }
            }

            WalkState::Continue
        })
    });

    pb.finish_and_clear();
    let duration = start.elapsed();

    println!("\nScan Result (completed in {:.2?}):", duration);
    println!("1. 文件总数量: {}", cnt.load(Ordering::Relaxed));
    println!(
        "2. 视频文件数量: {} (总大小: {})",
        video_cnt.load(Ordering::Relaxed),
        format_size(video_size_sum.load(Ordering::Relaxed) as u64)
    );
    println!(
        "3. 图片文件数量: {} (总大小: {})",
        image_cnt.load(Ordering::Relaxed),
        format_size(image_size_sum.load(Ordering::Relaxed) as u64)
    );
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes < KB {
        format!("{} B", bytes)
    } else if bytes < MB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else if bytes < GB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes < TB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    }
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
    let duplicate_size_sum = Arc::new(AtomicUsize::new(0));

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
        let duplicate_size_sum = duplicate_size_sum.clone();
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
                        match files.entry(id.to_uppercase()) {
                            dashmap::mapref::entry::Entry::Vacant(e) => {
                                e.insert(fullpath);
                            }
                            dashmap::mapref::entry::Entry::Occupied(e) => {
                                if !is_distinct_video_part(e.get(), &fullpath) {
                                    if let Ok(metadata) = entry.metadata() {
                                        duplicate_size_sum
                                            .fetch_add(metadata.len() as usize, Ordering::Relaxed);
                                    }
                                    let mut c = conflicts.lock().unwrap();
                                    c.push(fullpath);
                                }
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
    println!(
        "duplicate videos size: {}",
        format_size(duplicate_size_sum.load(Ordering::Relaxed) as u64)
    );

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
