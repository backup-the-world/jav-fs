use regex::Regex;
use rusqlite::{params, Connection};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrganizeCliPaths {
    pub source: Option<String>,
    pub target: Option<String>,
    pub database: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrganizeCliOptions {
    pub paths: OrganizeCliPaths,
    pub apply: bool,
    pub fail_fast: bool,
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrganizeOptions {
    pub source: PathBuf,
    pub target: PathBuf,
    pub database: PathBuf,
    pub apply: bool,
    pub fail_fast: bool,
    pub exclude: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct OrganizeConfigFile {
    organize: Option<OrganizeConfig>,
}

#[derive(Debug, Deserialize)]
struct OrganizeConfig {
    source: Option<String>,
    target: Option<String>,
    database: Option<String>,
    #[serde(default)]
    exclude: Vec<String>,
}

pub fn load_organize_options(
    cli: OrganizeCliOptions,
    config_path: impl Into<PathBuf>,
) -> Result<OrganizeOptions, String> {
    let config_path = config_path.into();
    let config = if config_path.exists() {
        Some(parse_organize_config(
            &std::fs::read_to_string(&config_path)
                .map_err(|e| format!("Failed to read config {}: {}", config_path.display(), e))?,
        )?)
    } else {
        None
    };

    resolve_organize_options(cli, config, home_dir())
}

fn parse_organize_config(content: &str) -> Result<OrganizeConfig, String> {
    let file: OrganizeConfigFile =
        toml::from_str(content).map_err(|e| format!("Failed to parse organize config: {}", e))?;
    file.organize
        .ok_or_else(|| "Missing [organize] config section".to_string())
}

fn resolve_organize_options(
    cli: OrganizeCliOptions,
    config: Option<OrganizeConfig>,
    home: Option<PathBuf>,
) -> Result<OrganizeOptions, String> {
    let cli_path_count = [
        cli.paths.source.as_ref(),
        cli.paths.target.as_ref(),
        cli.paths.database.as_ref(),
    ]
    .into_iter()
    .filter(|value| value.is_some())
    .count();

    if cli_path_count != 0 && cli_path_count != 3 {
        return Err(
            "Organize requires --source, --target and --database together; partial CLI paths are not allowed"
                .to_string(),
        );
    }

    let (source, target, database, mut exclude) = if cli_path_count == 3 {
        (
            cli.paths.source.unwrap(),
            cli.paths.target.unwrap(),
            cli.paths.database.unwrap(),
            config.map(|c| c.exclude).unwrap_or_default(),
        )
    } else {
        let config = config.ok_or_else(|| {
            "Organize requires complete CLI paths or a complete [organize] config".to_string()
        })?;
        let source = config
            .source
            .ok_or_else(|| "Organize config is missing source".to_string())?;
        let target = config
            .target
            .ok_or_else(|| "Organize config is missing target".to_string())?;
        let database = config
            .database
            .ok_or_else(|| "Organize config is missing database".to_string())?;
        (source, target, database, config.exclude)
    };

    exclude.extend(cli.exclude);

    Ok(OrganizeOptions {
        source: expand_tilde(&source, home.as_ref()),
        target: expand_tilde(&target, home.as_ref()),
        database: expand_tilde(&database, home.as_ref()),
        apply: cli.apply,
        fail_fast: cli.fail_fast,
        exclude,
    })
}

fn expand_tilde(path: &str, home: Option<&PathBuf>) -> PathBuf {
    match (path, home) {
        ("~", Some(home)) => home.clone(),
        (value, Some(home)) if value.starts_with("~/") => home.join(&value[2..]),
        _ => PathBuf::from(path),
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoCandidate {
    pub path: PathBuf,
    pub product_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OrganizeScanProgress {
    pub scanned_entries: usize,
    pub video_candidates: usize,
    pub unrecognized_videos: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrganizeProgress {
    Scan(OrganizeScanProgress),
    MetadataLookup { processed: usize, total: usize },
    Planning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActressMetadata {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VideoMetadata {
    pub product_id: String,
    pub title: String,
    pub release_date: String,
    pub actresses: Vec<ActressMetadata>,
    pub genres: Vec<String>,
    pub maker: Option<String>,
    pub label: Option<String>,
    pub series: Option<String>,
    pub duration: Option<i64>,
    pub description: Option<String>,
    pub cover_image: Option<String>,
    pub cover_image_landscape: Option<String>,
    pub cover_image_portrait: Option<String>,
    pub preview_images: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedVideo {
    pub source_path: PathBuf,
    pub target_video_path: PathBuf,
    pub nfo_path: PathBuf,
    pub poster_path: Option<PathBuf>,
    pub thumb_path: Option<PathBuf>,
    pub fanart_path: Option<PathBuf>,
    pub extrafanart_paths: Vec<PathBuf>,
    pub work_dir: PathBuf,
    pub actor_dir: PathBuf,
    pub metadata: VideoMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathWarning {
    pub product_id: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImageReferences {
    pub poster: Option<String>,
    pub thumb: Option<String>,
    pub fanart: Option<String>,
    pub extrafanart: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OrganizeReportCounts {
    pub planned_or_moved: usize,
    pub already_exists_skipped: usize,
    pub batch_conflicts: usize,
    pub target_name_conflicts: usize,
    pub missing_metadata: usize,
    pub missing_actresses: usize,
    pub missing_release_dates: usize,
    pub empty_titles: usize,
    pub nfo_failures: usize,
    pub artwork_warnings: usize,
    pub path_warnings: usize,
    pub unrecognized_videos: usize,
    pub source_delete_failures: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OrganizeDryRunReport {
    pub candidate_count: usize,
    pub candidates: Vec<VideoCandidate>,
    pub unrecognized_videos: Vec<PathBuf>,
    pub missing_metadata: Vec<VideoCandidate>,
    pub missing_actresses: Vec<VideoCandidate>,
    pub missing_release_dates: Vec<VideoCandidate>,
    pub empty_titles: Vec<VideoCandidate>,
    pub batch_conflicts: Vec<Vec<VideoCandidate>>,
    pub target_name_conflicts: Vec<VideoCandidate>,
    pub path_warnings: Vec<PathWarning>,
    pub artwork_warnings: Vec<String>,
    pub planned_videos: Vec<PlannedVideo>,
}

impl OrganizeDryRunReport {
    pub fn counts(&self) -> OrganizeReportCounts {
        OrganizeReportCounts {
            planned_or_moved: self.planned_videos.len(),
            already_exists_skipped: self.target_name_conflicts.len(),
            batch_conflicts: self.batch_conflicts.len(),
            target_name_conflicts: self.target_name_conflicts.len(),
            missing_metadata: self.missing_metadata.len(),
            missing_actresses: self.missing_actresses.len(),
            missing_release_dates: self.missing_release_dates.len(),
            empty_titles: self.empty_titles.len(),
            artwork_warnings: self.artwork_warnings.len(),
            path_warnings: self.path_warnings.len(),
            unrecognized_videos: self.unrecognized_videos.len(),
            ..Default::default()
        }
    }
}

pub fn run_organize_dry_run(options: &OrganizeOptions) -> Result<OrganizeDryRunReport, String> {
    run_organize_dry_run_with_progress(options, |_| {})
}

pub fn run_organize_dry_run_with_progress(
    options: &OrganizeOptions,
    mut progress: impl FnMut(OrganizeProgress),
) -> Result<OrganizeDryRunReport, String> {
    let mut report =
        scan_organize_source_with_progress(&options.source, &options.exclude, |scan| {
            progress(OrganizeProgress::Scan(scan));
        })?;
    let connection = Connection::open(&options.database).map_err(|e| {
        format!(
            "Failed to open database {}: {}",
            options.database.display(),
            e
        )
    })?;

    let total = report.candidates.len();
    progress(OrganizeProgress::MetadataLookup {
        processed: 0,
        total,
    });
    let mut known = Vec::new();
    let mut metadata = Vec::new();
    for (index, candidate) in report.candidates.into_iter().enumerate() {
        let processed = index + 1;
        if processed.is_multiple_of(100) || processed == total {
            progress(OrganizeProgress::MetadataLookup { processed, total });
        }
        match fetch_video_metadata(&connection, &candidate.product_id)? {
            Some(video_metadata) => {
                known.push(candidate);
                metadata.push(video_metadata);
            }
            None => report.missing_metadata.push(candidate),
        }
    }

    progress(OrganizeProgress::Planning);
    let mut planned = plan_organize_targets(&options.target, known.clone(), metadata);
    planned.candidate_count = report.candidate_count;
    planned.candidates = known;
    planned.unrecognized_videos = report.unrecognized_videos;
    planned.missing_metadata.extend(report.missing_metadata);
    Ok(planned)
}

pub fn scan_organize_source(
    source: &Path,
    exclude: &[String],
) -> Result<OrganizeDryRunReport, String> {
    scan_organize_source_with_progress(source, exclude, |_| {})
}

pub fn scan_organize_source_with_progress(
    source: &Path,
    exclude: &[String],
    mut progress: impl FnMut(OrganizeScanProgress),
) -> Result<OrganizeDryRunReport, String> {
    let mut report = OrganizeDryRunReport::default();
    let mut scan_progress = OrganizeScanProgress::default();
    scan_organize_dir(
        source,
        exclude,
        &mut report,
        &mut scan_progress,
        &mut progress,
    )?;
    report.candidate_count = report.candidates.len() + report.unrecognized_videos.len();
    progress(scan_progress);
    Ok(report)
}

fn scan_organize_dir(
    dir: &Path,
    exclude: &[String],
    report: &mut OrganizeDryRunReport,
    scan_progress: &mut OrganizeScanProgress,
    progress: &mut impl FnMut(OrganizeScanProgress),
) -> Result<(), String> {
    let entries =
        fs::read_dir(dir).map_err(|e| format!("Failed to read {}: {}", dir.display(), e))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        scan_progress.scanned_entries += 1;
        if scan_progress.scanned_entries.is_multiple_of(500) {
            progress(*scan_progress);
        }
        let path = entry.path();
        let filename = match path.file_name().and_then(|name| name.to_str()) {
            Some(filename) => filename,
            None => continue,
        };

        if is_hidden(filename) || is_excluded(filename, exclude) {
            continue;
        }

        if path.is_dir() {
            scan_organize_dir(&path, exclude, report, scan_progress, progress)?;
        } else if path.is_file() && is_video_file(filename) {
            if let Some(product_id) = extract_id_from_filename(filename) {
                scan_progress.video_candidates += 1;
                report.candidates.push(VideoCandidate {
                    path,
                    product_id: product_id.to_uppercase(),
                });
            } else {
                scan_progress.video_candidates += 1;
                scan_progress.unrecognized_videos += 1;
                report.unrecognized_videos.push(path);
            }
        }
    }
    Ok(())
}

fn is_hidden(filename: &str) -> bool {
    filename.starts_with('.')
}

fn is_excluded(filename: &str, exclude: &[String]) -> bool {
    const DEFAULT_EXCLUDES: &[&str] = &["@eaDir", "tmp", "temp", "incomplete"];
    DEFAULT_EXCLUDES
        .iter()
        .any(|default| filename.eq_ignore_ascii_case(default))
        || exclude.iter().any(|pattern| {
            let simple_name = pattern.trim_matches(|c| c == '*' || c == '/');
            filename == pattern || filename == simple_name
        })
}

pub fn plan_organize_targets(
    target: &Path,
    candidates: Vec<VideoCandidate>,
    metadata: Vec<VideoMetadata>,
) -> OrganizeDryRunReport {
    let metadata_by_id: BTreeMap<String, VideoMetadata> = metadata
        .into_iter()
        .map(|video| (video.product_id.to_uppercase(), video))
        .collect();
    let mut report = OrganizeDryRunReport::default();
    let mut groups: BTreeMap<String, Vec<VideoCandidate>> = BTreeMap::new();

    for candidate in candidates {
        groups
            .entry(candidate.product_id.to_uppercase())
            .or_default()
            .push(candidate);
    }

    for (product_id, group) in groups {
        let Some(video) = metadata_by_id.get(&product_id) else {
            report.missing_metadata.extend(group);
            continue;
        };

        if video.actresses.is_empty() {
            report.missing_actresses.extend(group);
            continue;
        }
        if video.release_date.trim().is_empty() {
            report.missing_release_dates.extend(group);
            continue;
        }
        if video.title.trim().is_empty() {
            report.empty_titles.extend(group);
            continue;
        }
        if has_batch_conflict(&group) {
            report.batch_conflicts.push(group);
            continue;
        }

        let actor_dir = actor_dir_for(target, &video.actresses);
        let year = video.release_date.chars().take(4).collect::<String>();
        let (title, title_changed) = sanitize_path_component(&video.title);
        let prefix = format!("[{}] {} - ", year, product_id);
        let (work_name, truncated) = fit_component(&prefix, &title, 180);
        if title_changed || truncated {
            report.path_warnings.push(PathWarning {
                product_id: product_id.clone(),
                message: "作品目录名已清洗或截断".to_string(),
            });
        }
        let work_dir = actor_dir.join(work_name);

        for candidate in group {
            let target_name = target_video_filename(&candidate, &product_id);
            let target_video_path = work_dir.join(target_name);
            if target_video_path.exists() {
                report.target_name_conflicts.push(candidate);
                continue;
            }
            let nfo_path = target_video_path.with_extension("nfo");
            let basename = target_video_path.with_extension("");
            let poster_path = video
                .cover_image_portrait
                .as_ref()
                .or(video.cover_image.as_ref())
                .map(|_| {
                    basename.with_file_name(format!(
                        "{}-poster.jpg",
                        basename.file_name().unwrap().to_string_lossy()
                    ))
                });
            let thumb_path = video.cover_image.as_ref().map(|_| {
                basename.with_file_name(format!(
                    "{}-thumb.jpg",
                    basename.file_name().unwrap().to_string_lossy()
                ))
            });
            let fanart_path = video.cover_image_landscape.as_ref().map(|_| {
                basename.with_file_name(format!(
                    "{}-fanart.jpg",
                    basename.file_name().unwrap().to_string_lossy()
                ))
            });
            let extrafanart_paths = video
                .preview_images
                .iter()
                .enumerate()
                .map(|(index, _)| {
                    work_dir
                        .join("extrafanart")
                        .join(format!("extrafanart-{}.jpg", index + 1))
                })
                .collect();
            report.planned_videos.push(PlannedVideo {
                source_path: candidate.path,
                target_video_path,
                nfo_path,
                poster_path,
                thumb_path,
                fanart_path,
                extrafanart_paths,
                work_dir: work_dir.clone(),
                actor_dir: actor_dir.clone(),
                metadata: video.clone(),
            });
        }
    }

    report
}

fn has_batch_conflict(group: &[VideoCandidate]) -> bool {
    if group.len() <= 1 {
        return false;
    }

    let mut parts = BTreeSet::new();
    for candidate in group {
        let Some(part) = extract_video_part_from_filename(&candidate.path.to_string_lossy()) else {
            return true;
        };
        if !parts.insert(part) {
            return true;
        }
    }
    false
}

fn actor_dir_for(target: &Path, actresses: &[ActressMetadata]) -> PathBuf {
    let expected_names = actor_names_by_id(actresses);
    let expected_set: BTreeSet<String> = expected_names.iter().cloned().collect();

    if let Ok(entries) = fs::read_dir(target) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let Some(stripped) = name.strip_prefix('#') else {
                continue;
            };
            let existing_set: BTreeSet<String> = stripped.split(',').map(str::to_string).collect();
            if existing_set == expected_set {
                return path;
            }
        }
    }

    target.join(format!("#{}", expected_names.join(",")))
}

fn actor_names_by_id(actresses: &[ActressMetadata]) -> Vec<String> {
    let mut actresses = actresses.to_vec();
    actresses.sort_by_key(|actress| actress.id);
    actresses
        .iter()
        .map(|actress| normalize_actor_dir_name(&actress.name))
        .collect()
}

fn normalize_actor_dir_name(name: &str) -> String {
    name.chars()
        .filter(|ch| *ch != ' ' && *ch != '　')
        .collect()
}

fn sanitize_path_component(value: &str) -> (String, bool) {
    let mut changed = false;
    let sanitized = value
        .chars()
        .map(|ch| {
            if matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
                changed = true;
                ' '
            } else {
                ch
            }
        })
        .collect::<String>()
        .trim()
        .to_string();
    (sanitized, changed)
}

fn fit_component(prefix: &str, title: &str, max_chars: usize) -> (String, bool) {
    let available = max_chars.saturating_sub(prefix.chars().count());
    if title.chars().count() <= available {
        return (format!("{}{}", prefix, title), false);
    }

    let truncated_title = title.chars().take(available).collect::<String>();
    (format!("{}{}", prefix, truncated_title), true)
}

fn target_video_filename(candidate: &VideoCandidate, product_id: &str) -> String {
    let extension = candidate
        .path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();
    let part = extract_video_part_from_filename(&candidate.path.to_string_lossy());
    match (part, extension.is_empty()) {
        (Some(part), false) => format!("{}-CD{}.{}", product_id, part, extension),
        (Some(part), true) => format!("{}-CD{}", product_id, part),
        (None, false) => format!("{}.{}", product_id, extension),
        (None, true) => product_id.to_string(),
    }
}

pub fn generate_nfo(metadata: &VideoMetadata, images: &ImageReferences) -> Result<String, String> {
    if metadata.product_id.trim().is_empty() || metadata.title.trim().is_empty() {
        return Err("NFO requires product_id and title".to_string());
    }

    let title = format!(
        "{} - {}",
        metadata.product_id.to_uppercase(),
        metadata.title
    );
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<movie>\n");
    push_element(&mut xml, "title", &title);
    push_element(&mut xml, "originaltitle", &title);
    push_element(&mut xml, "sorttitle", &title);

    for actress in &metadata.actresses {
        xml.push_str("  <actor>\n");
        push_element(&mut xml, "name", &actress.name);
        xml.push_str("  </actor>\n");
    }
    for genre in &metadata.genres {
        push_element(&mut xml, "tag", genre);
        push_element(&mut xml, "genre", genre);
    }
    if let Some(studio) = metadata.maker.as_ref().or(metadata.label.as_ref()) {
        push_element(&mut xml, "studio", studio);
    }
    if let Some(maker) = &metadata.maker {
        push_element(&mut xml, "maker", maker);
    }
    if let Some(label) = &metadata.label {
        push_element(&mut xml, "label", label);
    }
    if let Some(series) = &metadata.series {
        push_element(&mut xml, "set", series);
    }
    if let Some(duration) = metadata.duration {
        push_element(&mut xml, "runtime", &format!("{}分鍾", duration));
    }
    if let Some(description) = &metadata.description {
        push_element(&mut xml, "outline", description);
        push_element(&mut xml, "plot", description);
    }
    if let Some(poster) = &images.poster {
        push_element(&mut xml, "thumb", poster);
    }
    if let Some(thumb) = &images.thumb {
        push_element(&mut xml, "thumb", thumb);
    }
    if let Some(fanart) = &images.fanart {
        push_element(&mut xml, "fanart", fanart);
    }
    for image in &images.extrafanart {
        push_element(&mut xml, "fanart", image);
    }
    xml.push_str("</movie>\n");
    Ok(xml)
}

fn push_element(xml: &mut String, name: &str, value: &str) {
    xml.push_str("  <");
    xml.push_str(name);
    xml.push('>');
    xml.push_str(&escape_xml(value));
    xml.push_str("</");
    xml.push_str(name);
    xml.push_str(">\n");
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub trait ImageDownloader {
    fn download(&self, url: &str, destination: &Path) -> Result<(), String>;
}

pub struct HttpImageDownloader;

impl ImageDownloader for HttpImageDownloader {
    fn download(&self, url: &str, destination: &Path) -> Result<(), String> {
        let response = ureq::get(url)
            .set("User-Agent", "Mozilla/5.0 (compatible; jav-fs/1.0)")
            .call()
            .map_err(|e| format!("Failed to download {url}: {e}"))?;
        let mut reader = response.into_reader();
        let mut bytes = Vec::new();
        reader
            .read_to_end(&mut bytes)
            .map_err(|e| format!("Failed to read {url}: {e}"))?;
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create {}: {}", parent.display(), e))?;
        }
        fs::write(destination, bytes)
            .map_err(|e| format!("Failed to write {}: {}", destination.display(), e))
    }
}

pub fn prepare_artwork<D: ImageDownloader>(
    planned: &PlannedVideo,
    downloader: &D,
) -> (ImageReferences, Vec<String>) {
    let mut references = ImageReferences::default();
    let mut warnings = Vec::new();

    if let Some(path) = &planned.poster_path {
        let url = planned
            .metadata
            .cover_image_portrait
            .as_ref()
            .or(planned.metadata.cover_image.as_ref());
        download_artwork(url, path, downloader, &mut warnings, |reference| {
            references.poster = Some(reference)
        });
    }
    if let Some(path) = &planned.thumb_path {
        download_artwork(
            planned.metadata.cover_image.as_ref(),
            path,
            downloader,
            &mut warnings,
            |reference| references.thumb = Some(reference),
        );
    }
    if let Some(path) = &planned.fanart_path {
        download_artwork(
            planned.metadata.cover_image_landscape.as_ref(),
            path,
            downloader,
            &mut warnings,
            |reference| references.fanart = Some(reference),
        );
    }

    let extrafanart_dir = planned.work_dir.join("extrafanart");
    if !extrafanart_dir.exists() {
        for (url, path) in planned
            .metadata
            .preview_images
            .iter()
            .zip(planned.extrafanart_paths.iter())
        {
            download_artwork(Some(url), path, downloader, &mut warnings, |reference| {
                references.extrafanart.push(reference)
            });
        }
    }

    (references, warnings)
}

fn download_artwork<D: ImageDownloader>(
    url: Option<&String>,
    path: &Path,
    downloader: &D,
    warnings: &mut Vec<String>,
    mut record_reference: impl FnMut(String),
) {
    let Some(url) = url else { return };
    match downloader.download(url, path) {
        Ok(()) => record_reference(path.file_name().unwrap().to_string_lossy().to_string()),
        Err(e) => warnings.push(e),
    }
}

pub fn write_planned_nfos(planned_videos: &[PlannedVideo]) -> Result<Vec<PathBuf>, String> {
    let mut written = Vec::new();
    for planned in planned_videos {
        write_nfo(planned, &ImageReferences::default())?;
        written.push(planned.nfo_path.clone());
    }
    Ok(written)
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ApplyReport {
    pub moved_videos: Vec<PathBuf>,
    pub nfo_failures: Vec<PathBuf>,
    pub move_failures: Vec<PathBuf>,
    pub source_delete_failures: Vec<PathBuf>,
    pub artwork_warnings: Vec<String>,
}

impl ApplyReport {
    pub fn counts(&self) -> OrganizeReportCounts {
        OrganizeReportCounts {
            planned_or_moved: self.moved_videos.len(),
            nfo_failures: self.nfo_failures.len(),
            artwork_warnings: self.artwork_warnings.len(),
            source_delete_failures: self.source_delete_failures.len(),
            ..Default::default()
        }
    }
}

pub trait FileMover {
    fn rename(&self, source: &Path, destination: &Path) -> io::Result<()>;
    fn copy(&self, source: &Path, destination: &Path) -> io::Result<u64>;
    fn remove_file(&self, path: &Path) -> io::Result<()>;
    fn file_len(&self, path: &Path) -> io::Result<u64>;
}

pub struct StdFileMover;

impl FileMover for StdFileMover {
    fn rename(&self, source: &Path, destination: &Path) -> io::Result<()> {
        fs::rename(source, destination)
    }

    fn copy(&self, source: &Path, destination: &Path) -> io::Result<u64> {
        fs::copy(source, destination)
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        fs::remove_file(path)
    }

    fn file_len(&self, path: &Path) -> io::Result<u64> {
        fs::metadata(path).map(|metadata| metadata.len())
    }
}

pub fn apply_planned_videos<D: ImageDownloader, M: FileMover>(
    planned_videos: &[PlannedVideo],
    downloader: &D,
    mover: &M,
    fail_fast: bool,
) -> ApplyReport {
    let mut report = ApplyReport::default();

    for planned in planned_videos {
        let result = apply_one_planned_video(planned, downloader, mover, &mut report);
        if result.is_err() && fail_fast {
            break;
        }
    }

    report
}

fn apply_one_planned_video<D: ImageDownloader, M: FileMover>(
    planned: &PlannedVideo,
    downloader: &D,
    mover: &M,
    report: &mut ApplyReport,
) -> Result<(), ()> {
    let (images, mut image_warnings) = prepare_artwork(planned, downloader);
    report.artwork_warnings.append(&mut image_warnings);

    if write_nfo(planned, &images).is_err() {
        cleanup_prepared_files(planned);
        cleanup_empty_created_dirs(planned);
        report.nfo_failures.push(planned.source_path.clone());
        return Err(());
    }

    match move_video(planned, mover) {
        Ok(MoveOutcome::Moved) => {
            report.moved_videos.push(planned.target_video_path.clone());
            Ok(())
        }
        Ok(MoveOutcome::MovedWithSourceDeleteFailure) => {
            report.moved_videos.push(planned.target_video_path.clone());
            report
                .source_delete_failures
                .push(planned.source_path.clone());
            Ok(())
        }
        Err(()) => {
            cleanup_prepared_files(planned);
            cleanup_empty_created_dirs(planned);
            report.move_failures.push(planned.source_path.clone());
            Err(())
        }
    }
}

pub fn prepare_artwork_and_write_nfos<D: ImageDownloader>(
    planned_videos: &[PlannedVideo],
    downloader: &D,
) -> Result<(Vec<PathBuf>, Vec<String>), String> {
    let mut written = Vec::new();
    let mut warnings = Vec::new();
    for planned in planned_videos {
        let (images, mut image_warnings) = prepare_artwork(planned, downloader);
        warnings.append(&mut image_warnings);
        write_nfo(planned, &images)?;
        written.push(planned.nfo_path.clone());
    }
    Ok((written, warnings))
}

fn write_nfo(planned: &PlannedVideo, images: &ImageReferences) -> Result<(), String> {
    fs::create_dir_all(&planned.work_dir)
        .map_err(|e| format!("Failed to create {}: {}", planned.work_dir.display(), e))?;
    let content = generate_nfo(&planned.metadata, images)?;
    fs::write(&planned.nfo_path, content)
        .map_err(|e| format!("Failed to write {}: {}", planned.nfo_path.display(), e))
}

enum MoveOutcome {
    Moved,
    MovedWithSourceDeleteFailure,
}

fn move_video<M: FileMover>(planned: &PlannedVideo, mover: &M) -> Result<MoveOutcome, ()> {
    if let Some(parent) = planned.target_video_path.parent() {
        fs::create_dir_all(parent).map_err(|_| ())?;
    }

    match mover.rename(&planned.source_path, &planned.target_video_path) {
        Ok(()) => Ok(MoveOutcome::Moved),
        Err(_) => copy_then_delete(planned, mover),
    }
}

fn copy_then_delete<M: FileMover>(planned: &PlannedVideo, mover: &M) -> Result<MoveOutcome, ()> {
    mover
        .copy(&planned.source_path, &planned.target_video_path)
        .map_err(|_| ())?;
    let source_len = mover.file_len(&planned.source_path).map_err(|_| ())?;
    let target_len = mover.file_len(&planned.target_video_path).map_err(|_| ())?;
    if source_len != target_len {
        let _ = fs::remove_file(&planned.target_video_path);
        return Err(());
    }

    match mover.remove_file(&planned.source_path) {
        Ok(()) => Ok(MoveOutcome::Moved),
        Err(_) => Ok(MoveOutcome::MovedWithSourceDeleteFailure),
    }
}

fn cleanup_prepared_files(planned: &PlannedVideo) {
    let paths = [
        Some(planned.nfo_path.as_path()),
        planned.poster_path.as_deref(),
        planned.thumb_path.as_deref(),
        planned.fanart_path.as_deref(),
    ];
    for path in paths.into_iter().flatten() {
        let _ = fs::remove_file(path);
    }
    for path in &planned.extrafanart_paths {
        let _ = fs::remove_file(path);
    }
}

fn cleanup_empty_created_dirs(planned: &PlannedVideo) {
    let _ = fs::remove_dir(planned.work_dir.join("extrafanart"));
    let _ = fs::remove_dir(&planned.work_dir);
    let _ = fs::remove_dir(&planned.actor_dir);
}

fn fetch_video_metadata(
    connection: &Connection,
    product_id: &str,
) -> Result<Option<VideoMetadata>, String> {
    let mut statement = connection
        .prepare(
            "SELECT product_id, coalesce(title, ''), coalesce(release_date, '') \
             FROM videos WHERE product_id = ?1 LIMIT 1",
        )
        .map_err(|e| format!("Failed to prepare video metadata lookup: {}", e))?;
    let mut rows = statement
        .query(params![product_id])
        .map_err(|e| format!("Failed to query video metadata: {}", e))?;
    let Some(row) = rows
        .next()
        .map_err(|e| format!("Failed to read video metadata: {}", e))?
    else {
        return Ok(None);
    };

    let product_id = row
        .get::<_, String>(0)
        .map_err(|e| format!("Failed to read product_id: {}", e))?;
    let title = row
        .get::<_, String>(1)
        .map_err(|e| format!("Failed to read title: {}", e))?;
    let release_date = row
        .get::<_, String>(2)
        .map_err(|e| format!("Failed to read release_date: {}", e))?;
    let actresses = fetch_actresses(connection, &product_id)?;

    Ok(Some(VideoMetadata {
        product_id: product_id.clone(),
        title,
        release_date,
        actresses,
        genres: fetch_genres(connection, &product_id)?,
        maker: read_optional_text(connection, &product_id, "maker")?,
        label: read_optional_text(connection, &product_id, "label")?,
        series: read_optional_text(connection, &product_id, "series")?,
        duration: read_optional_i64(connection, &product_id, "duration")?,
        description: read_optional_text(connection, &product_id, "description")?,
        cover_image: read_optional_text(connection, &product_id, "cover_image")?,
        cover_image_landscape: read_optional_text(
            connection,
            &product_id,
            "cover_image_landscape",
        )?,
        cover_image_portrait: read_optional_text(connection, &product_id, "cover_image_portrait")?,
        preview_images: read_optional_text(connection, &product_id, "preview_images")?
            .map(|value| parse_preview_images(&value))
            .unwrap_or_default(),
    }))
}

fn parse_preview_images(value: &str) -> Vec<String> {
    if let Ok(images) = serde_json::from_str::<Vec<String>>(value) {
        return images;
    }

    value
        .split(['\n', ',', ';'])
        .map(str::trim)
        .map(|value| value.trim_matches(|ch| ch == '[' || ch == ']' || ch == '"'))
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

fn read_optional_text(
    connection: &Connection,
    product_id: &str,
    column: &str,
) -> Result<Option<String>, String> {
    let sql = format!("SELECT {column} FROM videos WHERE product_id = ?1 LIMIT 1");
    let mut statement = match connection.prepare(&sql) {
        Ok(statement) => statement,
        Err(_) => return Ok(None),
    };
    statement
        .query_row(params![product_id], |row| row.get::<_, Option<String>>(0))
        .map_err(|e| format!("Failed to read {column}: {}", e))
}

fn read_optional_i64(
    connection: &Connection,
    product_id: &str,
    column: &str,
) -> Result<Option<i64>, String> {
    let sql = format!("SELECT {column} FROM videos WHERE product_id = ?1 LIMIT 1");
    let mut statement = match connection.prepare(&sql) {
        Ok(statement) => statement,
        Err(_) => return Ok(None),
    };
    statement
        .query_row(params![product_id], |row| row.get::<_, Option<i64>>(0))
        .map_err(|e| format!("Failed to read {column}: {}", e))
}

fn fetch_genres(connection: &Connection, product_id: &str) -> Result<Vec<String>, String> {
    let mut statement = match connection.prepare(
        "SELECT genres.name \
         FROM genres \
         JOIN video_genres ON video_genres.genre_id = genres.id \
         WHERE video_genres.video_id = ?1 \
         ORDER BY genres.name",
    ) {
        Ok(statement) => statement,
        Err(_) => return Ok(Vec::new()),
    };

    let rows = statement
        .query_map(params![product_id], |row| row.get::<_, String>(0))
        .map_err(|e| format!("Failed to query genres: {}", e))?;

    let mut genres = Vec::new();
    for row in rows {
        genres.push(row.map_err(|e| format!("Failed to read genre: {}", e))?);
    }
    Ok(genres)
}

fn fetch_actresses(
    connection: &Connection,
    product_id: &str,
) -> Result<Vec<ActressMetadata>, String> {
    let mut statement = match connection.prepare(
        "SELECT actresses.id, actresses.name \
         FROM actresses \
         JOIN video_actresses ON video_actresses.actress_id = actresses.id \
         WHERE video_actresses.video_id = ?1 \
         ORDER BY actresses.id",
    ) {
        Ok(statement) => statement,
        Err(_) => return Ok(Vec::new()),
    };

    let rows = statement
        .query_map(params![product_id], |row| {
            Ok(ActressMetadata {
                id: row.get(0)?,
                name: row.get(1)?,
            })
        })
        .map_err(|e| format!("Failed to query actresses: {}", e))?;

    let mut actresses = Vec::new();
    for row in rows {
        actresses.push(row.map_err(|e| format!("Failed to read actress: {}", e))?);
    }
    Ok(actresses)
}

pub fn convert_smb_url_to_unc(url: &str) -> Result<String, String> {
    if !url.starts_with("smb://") {
        return Err("Not an SMB URL".to_string());
    }

    let parsed_url = Url::parse(url).map_err(|e| format!("Failed to parse URL: {}", e))?;

    let host = parsed_url
        .host_str()
        .ok_or("Missing host in URL")?
        .to_string();

    let mut path_parts = Vec::new();
    if let Some(segments) = parsed_url.path_segments() {
        for segment in segments {
            if !segment.is_empty() {
                path_parts.push(segment);
            }
        }
    }

    let unc_path = if path_parts.is_empty() {
        format!("\\\\{}", host)
    } else {
        format!("\\\\{}\\{}", host, path_parts.join("\\"))
    };

    Ok(unc_path)
}

const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mkv", "wmv", "avi", "mov", "m4v", "ts"];
const VIDEO_EXTENSION_PATTERN: &str = r".*\.(?i)(mp4|mkv|wmv|avi|mov|m4v|ts)$";

pub fn is_video_file(filename: &str) -> bool {
    let lower = filename.to_lowercase();
    VIDEO_EXTENSIONS
        .iter()
        .any(|extension| lower.ends_with(&format!(".{extension}")))
}

pub fn is_image_file(filename: &str) -> bool {
    let lower = filename.to_lowercase();
    lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".png")
        || lower.ends_with(".gif")
        || lower.ends_with(".bmp")
        || lower.ends_with(".webp")
}

pub fn extract_id_from_filename(filename: &str) -> Option<String> {
    static RE_VIDEO: OnceLock<Regex> = OnceLock::new();
    static RE_ID_WITH_DASH: OnceLock<Regex> = OnceLock::new();
    static RE_ID_WITHOUT_DASH: OnceLock<Regex> = OnceLock::new();

    let re_video = RE_VIDEO.get_or_init(|| Regex::new(VIDEO_EXTENSION_PATTERN).unwrap());
    let re_id_with_dash = RE_ID_WITH_DASH
        .get_or_init(|| Regex::new(r"[[:alnum:]]*[[:alpha:]][[:alnum:]]*-\d+").unwrap());
    let re_id_without_dash =
        RE_ID_WITHOUT_DASH.get_or_init(|| Regex::new(r"[[:alpha:]]+\d+").unwrap());

    let name_without_ext = if re_video.is_match(filename) {
        let pos = filename.rfind('.').unwrap();
        &filename[..pos]
    } else {
        filename
    };

    let find_id = |name: &str| {
        re_id_with_dash
            .find(name)
            .or_else(|| re_id_without_dash.find(name))
            .map(|m| m.as_str().to_string())
    };

    name_without_ext
        .rsplit_once('@')
        .and_then(|(_, suffix)| find_id(suffix))
        .or_else(|| find_id(name_without_ext))
}

pub fn extract_prefix_from_id(id: &str) -> Option<String> {
    static RE_PREFIX: OnceLock<Regex> = OnceLock::new();
    let re_prefix = RE_PREFIX.get_or_init(|| Regex::new(r"^[[:alpha:]]+").unwrap());
    re_prefix.find(id).map(|m| m.as_str().to_string())
}

pub fn extract_video_part_from_filename(filename: &str) -> Option<String> {
    static RE_VIDEO: OnceLock<Regex> = OnceLock::new();
    static RE_PART: OnceLock<Regex> = OnceLock::new();

    let re_video = RE_VIDEO.get_or_init(|| Regex::new(VIDEO_EXTENSION_PATTERN).unwrap());
    let re_part = RE_PART.get_or_init(|| {
        Regex::new(
            r"(?i)(?:[_\. ](?:part|pt|cd)?(\d+)|-(?:part|pt|cd)(\d+))(?:[_\-. ]?(?:4k|8k|fhd|hd))?$",
        )
        .unwrap()
    });

    let name_without_ext = if re_video.is_match(filename) {
        let pos = filename.rfind('.').unwrap();
        &filename[..pos]
    } else {
        filename
    };

    re_part.captures(name_without_ext).and_then(|captures| {
        captures
            .get(1)
            .or_else(|| captures.get(2))
            .map(|m| m.as_str().to_string())
    })
}

pub fn is_distinct_video_part(left: &str, right: &str) -> bool {
    match (
        extract_video_part_from_filename(left),
        extract_video_part_from_filename(right),
    ) {
        (Some(left_part), Some(right_part)) => left_part != right_part,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cli_paths(
        source: Option<&str>,
        target: Option<&str>,
        database: Option<&str>,
    ) -> OrganizeCliPaths {
        OrganizeCliPaths {
            source: source.map(str::to_string),
            target: target.map(str::to_string),
            database: database.map(str::to_string),
        }
    }

    use rusqlite::Connection;
    use std::fs::{self, File};
    use tempfile::tempdir;

    fn cli_options(paths: OrganizeCliPaths) -> OrganizeCliOptions {
        OrganizeCliOptions {
            paths,
            apply: false,
            fail_fast: false,
            exclude: Vec::new(),
        }
    }

    #[test]
    fn test_resolve_organize_options_complete_cli_without_config() {
        let options = resolve_organize_options(
            cli_options(cli_paths(
                Some("/incoming"),
                Some("/media"),
                Some("~/db.sqlite"),
            )),
            None,
            Some(PathBuf::from("/home/me")),
        )
        .unwrap();

        assert_eq!(options.source, PathBuf::from("/incoming"));
        assert_eq!(options.target, PathBuf::from("/media"));
        assert_eq!(options.database, PathBuf::from("/home/me/db.sqlite"));
        assert!(!options.apply);
    }

    #[test]
    fn test_resolve_organize_options_complete_config_without_cli_paths() {
        let config = parse_organize_config(
            r#"
[organize]
source = "~/incoming"
target = "/media"
database = "/db.sqlite"
exclude = ["tmp"]
"#,
        )
        .unwrap();

        let options = resolve_organize_options(
            cli_options(cli_paths(None, None, None)),
            Some(config),
            Some(PathBuf::from("/home/me")),
        )
        .unwrap();

        assert_eq!(options.source, PathBuf::from("/home/me/incoming"));
        assert_eq!(options.exclude, vec!["tmp"]);
    }

    #[test]
    fn test_resolve_organize_options_complete_cli_overrides_config_paths_and_appends_exclude() {
        let config = parse_organize_config(
            r#"
[organize]
source = "/config-incoming"
target = "/config-media"
database = "/config.sqlite"
exclude = ["config-exclude"]
"#,
        )
        .unwrap();
        let mut cli = cli_options(cli_paths(
            Some("/cli-incoming"),
            Some("/cli-media"),
            Some("/cli.sqlite"),
        ));
        cli.exclude.push("cli-exclude".to_string());
        cli.apply = true;
        cli.fail_fast = true;

        let options =
            resolve_organize_options(cli, Some(config), Some(PathBuf::from("/home/me"))).unwrap();

        assert_eq!(options.source, PathBuf::from("/cli-incoming"));
        assert_eq!(options.target, PathBuf::from("/cli-media"));
        assert_eq!(options.database, PathBuf::from("/cli.sqlite"));
        assert_eq!(options.exclude, vec!["config-exclude", "cli-exclude"]);
        assert!(options.apply);
        assert!(options.fail_fast);
    }

    #[test]
    fn test_resolve_organize_options_rejects_partial_cli_even_with_complete_config() {
        let config = parse_organize_config(
            r#"
[organize]
source = "/config-incoming"
target = "/config-media"
database = "/config.sqlite"
"#,
        )
        .unwrap();

        let result = resolve_organize_options(
            cli_options(cli_paths(Some("/cli-incoming"), None, None)),
            Some(config),
            Some(PathBuf::from("/home/me")),
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_organize_options_rejects_incomplete_config() {
        let config = parse_organize_config(
            r#"
[organize]
source = "/config-incoming"
target = "/config-media"
"#,
        )
        .unwrap();

        let result = resolve_organize_options(
            cli_options(cli_paths(None, None, None)),
            Some(config),
            Some(PathBuf::from("/home/me")),
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_organize_options_does_not_expand_environment_variables() {
        let options = resolve_organize_options(
            cli_options(cli_paths(
                Some("$SRC"),
                Some("${TARGET}"),
                Some("~/db.sqlite"),
            )),
            None,
            Some(PathBuf::from("/home/me")),
        )
        .unwrap();

        assert_eq!(options.source, PathBuf::from("$SRC"));
        assert_eq!(options.target, PathBuf::from("${TARGET}"));
        assert_eq!(options.database, PathBuf::from("/home/me/db.sqlite"));
    }

    #[test]
    fn test_scan_organize_source_skips_hidden_and_excluded_paths() {
        let dir = tempdir().unwrap();
        File::create(dir.path().join("ABC-123.mp4")).unwrap();
        File::create(dir.path().join("note.txt")).unwrap();
        File::create(dir.path().join("UNKNOWN.avi")).unwrap();
        fs::create_dir(dir.path().join(".hidden")).unwrap();
        File::create(dir.path().join(".hidden").join("DEF-456.mp4")).unwrap();
        fs::create_dir(dir.path().join("tmp")).unwrap();
        File::create(dir.path().join("tmp").join("GHI-789.mp4")).unwrap();
        fs::create_dir(dir.path().join("custom-skip")).unwrap();
        File::create(dir.path().join("custom-skip").join("JKL-111.mp4")).unwrap();

        let report = scan_organize_source(dir.path(), &["**/custom-skip/**".to_string()]).unwrap();

        assert_eq!(report.candidate_count, 2);
        assert_eq!(report.candidates.len(), 1);
        assert_eq!(report.candidates[0].product_id, "ABC-123");
        assert_eq!(report.unrecognized_videos.len(), 1);
        assert!(report.unrecognized_videos[0].ends_with("UNKNOWN.avi"));
    }

    #[test]
    fn test_run_organize_dry_run_reports_missing_metadata() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("incoming");
        fs::create_dir(&source).unwrap();
        File::create(source.join("ABC-123.mp4")).unwrap();
        File::create(source.join("DEF-456.mkv")).unwrap();
        File::create(source.join("video.mov")).unwrap();

        let database = dir.path().join("jav-data.db");
        let connection = Connection::open(&database).unwrap();
        connection
            .execute(
                "CREATE TABLE videos (product_id TEXT NOT NULL, title TEXT, release_date TEXT)",
                [],
            )
            .unwrap();
        connection
            .execute(
                "CREATE TABLE actresses (id INTEGER NOT NULL, name TEXT NOT NULL)",
                [],
            )
            .unwrap();
        connection
            .execute(
                "CREATE TABLE video_actresses (video_id TEXT NOT NULL, actress_id INTEGER NOT NULL)",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO videos (product_id, title, release_date) VALUES ('ABC-123', '标题', '2024-05-01')",
                [],
            )
            .unwrap();
        connection
            .execute("INSERT INTO actresses (id, name) VALUES (1, '演员')", [])
            .unwrap();
        connection
            .execute(
                "INSERT INTO video_actresses (video_id, actress_id) VALUES ('ABC-123', 1)",
                [],
            )
            .unwrap();
        drop(connection);

        let options = OrganizeOptions {
            source,
            target: dir.path().join("media"),
            database,
            apply: false,
            fail_fast: false,
            exclude: Vec::new(),
        };

        let report = run_organize_dry_run(&options).unwrap();

        assert_eq!(report.candidate_count, 3);
        assert_eq!(report.candidates.len(), 1);
        assert_eq!(report.candidates[0].product_id, "ABC-123");
        assert_eq!(report.missing_metadata.len(), 1);
        assert_eq!(report.missing_metadata[0].product_id, "DEF-456");
        assert_eq!(report.unrecognized_videos.len(), 1);
        assert!(
            !options.target.exists(),
            "dry-run must not create target directory"
        );
    }

    #[test]
    fn test_plan_organize_targets_builds_actor_work_and_video_paths() {
        let dir = tempdir().unwrap();
        let candidate = VideoCandidate {
            path: dir.path().join("hhd800.com@abc-123.mp4"),
            product_id: "abc-123".to_string(),
        };
        let metadata = VideoMetadata {
            product_id: "ABC-123".to_string(),
            title: "好看的:标题".to_string(),
            release_date: "2024-05-01".to_string(),
            actresses: vec![
                ActressMetadata {
                    id: 2,
                    name: "乙 女".to_string(),
                },
                ActressMetadata {
                    id: 1,
                    name: "甲　女".to_string(),
                },
            ],
            ..Default::default()
        };

        let report = plan_organize_targets(dir.path(), vec![candidate], vec![metadata]);

        assert_eq!(report.planned_videos.len(), 1);
        let planned = &report.planned_videos[0];
        assert!(planned.actor_dir.ends_with("#甲女,乙女"));
        assert!(planned.work_dir.ends_with("[2024] ABC-123 - 好看的 标题"));
        assert!(planned.target_video_path.ends_with("ABC-123.mp4"));
        assert_eq!(report.path_warnings.len(), 1);
    }

    #[test]
    fn test_plan_organize_targets_reuses_existing_actor_collection_dir() {
        let dir = tempdir().unwrap();
        fs::create_dir(dir.path().join("#乙女,甲女")).unwrap();
        let candidate = VideoCandidate {
            path: dir.path().join("ABC-123.mp4"),
            product_id: "ABC-123".to_string(),
        };
        let metadata = VideoMetadata {
            product_id: "ABC-123".to_string(),
            title: "标题".to_string(),
            release_date: "2024-05-01".to_string(),
            actresses: vec![
                ActressMetadata {
                    id: 1,
                    name: "甲女".to_string(),
                },
                ActressMetadata {
                    id: 2,
                    name: "乙女".to_string(),
                },
            ],
            ..Default::default()
        };

        let report = plan_organize_targets(dir.path(), vec![candidate], vec![metadata]);

        assert!(report.planned_videos[0].actor_dir.ends_with("#乙女,甲女"));
    }

    #[test]
    fn test_plan_organize_targets_names_distinct_parts_and_rejects_ambiguous_batch() {
        let dir = tempdir().unwrap();
        let metadata = VideoMetadata {
            product_id: "ABC-123".to_string(),
            title: "标题".to_string(),
            release_date: "2024-05-01".to_string(),
            actresses: vec![ActressMetadata {
                id: 1,
                name: "演员".to_string(),
            }],
            ..Default::default()
        };
        let part_report = plan_organize_targets(
            dir.path(),
            vec![
                VideoCandidate {
                    path: dir.path().join("ABC-123-CD1.mkv"),
                    product_id: "ABC-123".to_string(),
                },
                VideoCandidate {
                    path: dir.path().join("ABC-123-CD2.mkv"),
                    product_id: "ABC-123".to_string(),
                },
            ],
            vec![metadata.clone()],
        );
        let target_names: BTreeSet<_> = part_report
            .planned_videos
            .iter()
            .map(|video| {
                video
                    .target_video_path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string()
            })
            .collect();
        assert_eq!(
            target_names,
            BTreeSet::from(["ABC-123-CD1.mkv".to_string(), "ABC-123-CD2.mkv".to_string()])
        );

        let conflict_report = plan_organize_targets(
            dir.path(),
            vec![
                VideoCandidate {
                    path: dir.path().join("ABC-123-A.mp4"),
                    product_id: "ABC-123".to_string(),
                },
                VideoCandidate {
                    path: dir.path().join("site@ABC-123.mp4"),
                    product_id: "ABC-123".to_string(),
                },
            ],
            vec![metadata],
        );
        assert_eq!(conflict_report.batch_conflicts.len(), 1);
        assert!(conflict_report.planned_videos.is_empty());
    }

    #[test]
    fn test_plan_organize_targets_reports_required_metadata_gaps() {
        let dir = tempdir().unwrap();
        let report = plan_organize_targets(
            dir.path(),
            vec![
                VideoCandidate {
                    path: dir.path().join("DATE-001.mp4"),
                    product_id: "DATE-001".to_string(),
                },
                VideoCandidate {
                    path: dir.path().join("TITLE-001.mp4"),
                    product_id: "TITLE-001".to_string(),
                },
            ],
            vec![
                VideoMetadata {
                    product_id: "DATE-001".to_string(),
                    title: "标题".to_string(),
                    release_date: "".to_string(),
                    actresses: vec![ActressMetadata {
                        id: 1,
                        name: "演员".to_string(),
                    }],
                    ..Default::default()
                },
                VideoMetadata {
                    product_id: "TITLE-001".to_string(),
                    title: "".to_string(),
                    release_date: "2024-05-01".to_string(),
                    actresses: vec![ActressMetadata {
                        id: 1,
                        name: "演员".to_string(),
                    }],
                    ..Default::default()
                },
            ],
        );

        assert_eq!(report.missing_release_dates.len(), 1);
        assert_eq!(report.empty_titles.len(), 1);
        assert!(report.planned_videos.is_empty());
    }

    #[test]
    fn test_plan_organize_targets_reports_metadata_and_target_conflicts() {
        let dir = tempdir().unwrap();
        let work_dir = dir.path().join("#演员").join("[2024] ABC-123 - 标题");
        fs::create_dir_all(&work_dir).unwrap();
        File::create(work_dir.join("ABC-123.mp4")).unwrap();

        let report = plan_organize_targets(
            dir.path(),
            vec![
                VideoCandidate {
                    path: dir.path().join("ABC-123.mp4"),
                    product_id: "ABC-123".to_string(),
                },
                VideoCandidate {
                    path: dir.path().join("NOPE-404.mp4"),
                    product_id: "NOPE-404".to_string(),
                },
                VideoCandidate {
                    path: dir.path().join("ACT-000.mp4"),
                    product_id: "ACT-000".to_string(),
                },
            ],
            vec![
                VideoMetadata {
                    product_id: "ABC-123".to_string(),
                    title: "标题".to_string(),
                    release_date: "2024-05-01".to_string(),
                    actresses: vec![ActressMetadata {
                        id: 1,
                        name: "演员".to_string(),
                    }],
                    ..Default::default()
                },
                VideoMetadata {
                    product_id: "ACT-000".to_string(),
                    title: "标题".to_string(),
                    release_date: "2024-05-01".to_string(),
                    actresses: Vec::new(),
                    ..Default::default()
                },
            ],
        );

        assert_eq!(report.target_name_conflicts.len(), 1);
        assert_eq!(report.missing_metadata.len(), 1);
        assert_eq!(report.missing_actresses.len(), 1);
        assert!(report.planned_videos.is_empty());
    }

    #[test]
    fn test_generate_nfo_maps_fields_and_escapes_text() {
        let metadata = VideoMetadata {
            product_id: "abc-123".to_string(),
            title: "标题 & <测试>".to_string(),
            actresses: vec![ActressMetadata {
                id: 1,
                name: "演 员".to_string(),
            }],
            genres: vec!["剧情".to_string()],
            maker: Some("Maker".to_string()),
            label: Some("Label".to_string()),
            series: Some("Series".to_string()),
            duration: Some(120),
            description: Some("简介 & 剧情".to_string()),
            ..Default::default()
        };
        let images = ImageReferences {
            poster: Some("ABC-123-poster.jpg".to_string()),
            thumb: Some("ABC-123-thumb.jpg".to_string()),
            fanart: None,
            extrafanart: Vec::new(),
        };

        let xml = generate_nfo(&metadata, &images).unwrap();

        assert!(xml.contains("<title>ABC-123 - 标题 &amp; &lt;测试&gt;</title>"));
        assert!(xml.contains("<originaltitle>ABC-123 - 标题 &amp; &lt;测试&gt;</originaltitle>"));
        assert!(xml.contains("<sorttitle>ABC-123 - 标题 &amp; &lt;测试&gt;</sorttitle>"));
        assert!(xml.contains("<name>演 员</name>"));
        assert!(xml.contains("<tag>剧情</tag>"));
        assert!(xml.contains("<genre>剧情</genre>"));
        assert!(xml.contains("<studio>Maker</studio>"));
        assert!(xml.contains("<maker>Maker</maker>"));
        assert!(xml.contains("<label>Label</label>"));
        assert!(xml.contains("<set>Series</set>"));
        assert!(xml.contains("<runtime>120分鍾</runtime>"));
        assert!(xml.contains("<outline>简介 &amp; 剧情</outline>"));
        assert!(xml.contains("<plot>简介 &amp; 剧情</plot>"));
        assert!(xml.contains("ABC-123-poster.jpg"));
        assert!(xml.contains("ABC-123-thumb.jpg"));
    }

    #[test]
    fn test_generate_nfo_uses_label_as_studio_without_maker_and_only_given_images() {
        let metadata = VideoMetadata {
            product_id: "ABC-123".to_string(),
            title: "标题".to_string(),
            label: Some("Label".to_string()),
            ..Default::default()
        };
        let images = ImageReferences {
            fanart: Some("ABC-123-fanart.jpg".to_string()),
            ..Default::default()
        };

        let xml = generate_nfo(&metadata, &images).unwrap();

        assert!(xml.contains("<studio>Label</studio>"));
        assert!(!xml.contains("poster"));
        assert!(xml.contains("ABC-123-fanart.jpg"));
    }

    struct FakeDownloader {
        fail_url: Option<String>,
    }

    struct FakeMover {
        rename_fails: bool,
        copied_len: Option<u64>,
        delete_fails: bool,
    }

    impl FileMover for FakeMover {
        fn rename(&self, source: &Path, destination: &Path) -> io::Result<()> {
            if self.rename_fails {
                return Err(io::Error::other("rename failed"));
            }
            fs::rename(source, destination)
        }

        fn copy(&self, source: &Path, destination: &Path) -> io::Result<u64> {
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)?;
            }
            let len = fs::copy(source, destination)?;
            if let Some(copied_len) = self.copied_len {
                let file = fs::OpenOptions::new().write(true).open(destination)?;
                file.set_len(copied_len)?;
                Ok(copied_len)
            } else {
                Ok(len)
            }
        }

        fn remove_file(&self, path: &Path) -> io::Result<()> {
            if self.delete_fails {
                return Err(io::Error::other("delete failed"));
            }
            fs::remove_file(path)
        }

        fn file_len(&self, path: &Path) -> io::Result<u64> {
            fs::metadata(path).map(|metadata| metadata.len())
        }
    }

    impl ImageDownloader for FakeDownloader {
        fn download(&self, url: &str, destination: &Path) -> Result<(), String> {
            if self.fail_url.as_deref() == Some(url) {
                return Err(format!("failed {url}"));
            }
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(destination, url).unwrap();
            Ok(())
        }
    }

    #[test]
    fn test_parse_preview_images_reads_json_array_urls() {
        let images =
            parse_preview_images(r#"["https://example.test/1.jpg", "https://example.test/2.jpg"]"#);

        assert_eq!(
            images,
            vec![
                "https://example.test/1.jpg".to_string(),
                "https://example.test/2.jpg".to_string()
            ]
        );
    }

    #[test]
    fn test_prepare_artwork_maps_urls_and_keeps_only_successful_references() {
        let dir = tempdir().unwrap();
        let candidate = VideoCandidate {
            path: dir.path().join("ABC-123-CD1.mp4"),
            product_id: "ABC-123".to_string(),
        };
        let metadata = VideoMetadata {
            product_id: "ABC-123".to_string(),
            title: "标题".to_string(),
            release_date: "2024-05-01".to_string(),
            actresses: vec![ActressMetadata {
                id: 1,
                name: "演员".to_string(),
            }],
            cover_image: Some("cover".to_string()),
            cover_image_portrait: Some("portrait".to_string()),
            cover_image_landscape: Some("landscape".to_string()),
            preview_images: vec!["preview1".to_string(), "bad".to_string()],
            ..Default::default()
        };
        let report = plan_organize_targets(dir.path(), vec![candidate], vec![metadata]);
        let planned = &report.planned_videos[0];

        let (images, warnings) = prepare_artwork(
            planned,
            &FakeDownloader {
                fail_url: Some("bad".to_string()),
            },
        );

        assert_eq!(images.poster, Some("ABC-123-CD1-poster.jpg".to_string()));
        assert_eq!(images.thumb, Some("ABC-123-CD1-thumb.jpg".to_string()));
        assert_eq!(images.fanart, Some("ABC-123-CD1-fanart.jpg".to_string()));
        assert_eq!(images.extrafanart, vec!["extrafanart-1.jpg"]);
        assert_eq!(warnings.len(), 1);
        assert!(planned.poster_path.as_ref().unwrap().exists());
        assert!(planned.thumb_path.as_ref().unwrap().exists());
        assert!(planned.fanart_path.as_ref().unwrap().exists());
    }

    #[test]
    fn test_prepare_artwork_skips_existing_extrafanart_dir() {
        let dir = tempdir().unwrap();
        let candidate = VideoCandidate {
            path: dir.path().join("ABC-123.mp4"),
            product_id: "ABC-123".to_string(),
        };
        let metadata = VideoMetadata {
            product_id: "ABC-123".to_string(),
            title: "标题".to_string(),
            release_date: "2024-05-01".to_string(),
            actresses: vec![ActressMetadata {
                id: 1,
                name: "演员".to_string(),
            }],
            preview_images: vec!["preview1".to_string()],
            ..Default::default()
        };
        let report = plan_organize_targets(dir.path(), vec![candidate], vec![metadata]);
        let planned = &report.planned_videos[0];
        fs::create_dir_all(planned.work_dir.join("extrafanart")).unwrap();

        let (images, warnings) = prepare_artwork(planned, &FakeDownloader { fail_url: None });

        assert!(images.extrafanart.is_empty());
        assert!(warnings.is_empty());
        assert!(!planned.extrafanart_paths[0].exists());
    }

    #[test]
    fn test_prepare_artwork_and_write_nfos_references_successful_images() {
        let dir = tempdir().unwrap();
        let candidate = VideoCandidate {
            path: dir.path().join("ABC-123.mp4"),
            product_id: "ABC-123".to_string(),
        };
        let metadata = VideoMetadata {
            product_id: "ABC-123".to_string(),
            title: "标题".to_string(),
            release_date: "2024-05-01".to_string(),
            actresses: vec![ActressMetadata {
                id: 1,
                name: "演员".to_string(),
            }],
            cover_image: Some("cover".to_string()),
            cover_image_landscape: Some("bad".to_string()),
            ..Default::default()
        };
        let report = plan_organize_targets(dir.path(), vec![candidate], vec![metadata]);

        let (written, warnings) = prepare_artwork_and_write_nfos(
            &report.planned_videos,
            &FakeDownloader {
                fail_url: Some("bad".to_string()),
            },
        )
        .unwrap();

        let xml = fs::read_to_string(&written[0]).unwrap();
        assert!(xml.contains("ABC-123-poster.jpg"));
        assert!(xml.contains("ABC-123-thumb.jpg"));
        assert!(!xml.contains("ABC-123-fanart.jpg"));
        assert_eq!(warnings.len(), 1);
    }

    fn planned_for_apply(dir: &Path, product_id: &str) -> PlannedVideo {
        let source_path = dir.join(format!("{product_id}.mp4"));
        fs::write(&source_path, "video").unwrap();
        let work_dir = dir
            .join("#演员")
            .join(format!("[2024] {product_id} - 标题"));
        PlannedVideo {
            source_path,
            target_video_path: work_dir.join(format!("{product_id}.mp4")),
            nfo_path: work_dir.join(format!("{product_id}.nfo")),
            poster_path: Some(work_dir.join(format!("{product_id}-poster.jpg"))),
            thumb_path: None,
            fanart_path: None,
            extrafanart_paths: Vec::new(),
            work_dir,
            actor_dir: dir.join("#演员"),
            metadata: VideoMetadata {
                product_id: product_id.to_string(),
                title: "标题".to_string(),
                cover_image: Some("cover".to_string()),
                ..Default::default()
            },
        }
    }

    #[test]
    fn test_report_counts_are_structured_not_human_string_dependent() {
        let dry_report = OrganizeDryRunReport {
            planned_videos: Vec::new(),
            target_name_conflicts: vec![VideoCandidate {
                path: PathBuf::from("exists.mp4"),
                product_id: "ABC-123".to_string(),
            }],
            missing_metadata: vec![VideoCandidate {
                path: PathBuf::from("missing.mp4"),
                product_id: "DEF-456".to_string(),
            }],
            unrecognized_videos: vec![PathBuf::from("video.mp4")],
            path_warnings: vec![PathWarning {
                product_id: "ABC-123".to_string(),
                message: "cleaned".to_string(),
            }],
            ..Default::default()
        };
        let dry_counts = dry_report.counts();
        assert_eq!(dry_counts.already_exists_skipped, 1);
        assert_eq!(dry_counts.target_name_conflicts, 1);
        assert_eq!(dry_counts.missing_metadata, 1);
        assert_eq!(dry_counts.unrecognized_videos, 1);
        assert_eq!(dry_counts.path_warnings, 1);

        let apply_report = ApplyReport {
            moved_videos: vec![PathBuf::from("target.mp4")],
            nfo_failures: vec![PathBuf::from("bad.mp4")],
            source_delete_failures: vec![PathBuf::from("source.mp4")],
            artwork_warnings: vec!["warn".to_string()],
            ..Default::default()
        };
        let apply_counts = apply_report.counts();
        assert_eq!(apply_counts.planned_or_moved, 1);
        assert_eq!(apply_counts.nfo_failures, 1);
        assert_eq!(apply_counts.source_delete_failures, 1);
        assert_eq!(apply_counts.artwork_warnings, 1);
    }

    #[test]
    fn test_apply_planned_videos_renames_after_artwork_and_nfo() {
        let dir = tempdir().unwrap();
        let planned = planned_for_apply(dir.path(), "ABC-123");

        let report = apply_planned_videos(
            std::slice::from_ref(&planned),
            &FakeDownloader { fail_url: None },
            &FakeMover {
                rename_fails: false,
                copied_len: None,
                delete_fails: false,
            },
            false,
        );

        assert_eq!(report.moved_videos, vec![planned.target_video_path.clone()]);
        assert!(!planned.source_path.exists());
        assert!(planned.target_video_path.exists());
        assert!(planned.nfo_path.exists());
        assert!(planned.poster_path.unwrap().exists());
    }

    #[test]
    fn test_apply_planned_videos_falls_back_to_copy_and_delete() {
        let dir = tempdir().unwrap();
        let planned = planned_for_apply(dir.path(), "ABC-123");

        let report = apply_planned_videos(
            std::slice::from_ref(&planned),
            &FakeDownloader { fail_url: None },
            &FakeMover {
                rename_fails: true,
                copied_len: None,
                delete_fails: false,
            },
            false,
        );

        assert_eq!(report.moved_videos.len(), 1);
        assert!(!planned.source_path.exists());
        assert!(planned.target_video_path.exists());
    }

    #[test]
    fn test_apply_planned_videos_reports_delete_failure_as_success_warning() {
        let dir = tempdir().unwrap();
        let planned = planned_for_apply(dir.path(), "ABC-123");

        let report = apply_planned_videos(
            std::slice::from_ref(&planned),
            &FakeDownloader { fail_url: None },
            &FakeMover {
                rename_fails: true,
                copied_len: None,
                delete_fails: true,
            },
            false,
        );

        assert_eq!(report.moved_videos.len(), 1);
        assert_eq!(
            report.source_delete_failures,
            vec![planned.source_path.clone()]
        );
        assert!(planned.source_path.exists());
        assert!(planned.target_video_path.exists());
    }

    #[test]
    fn test_apply_planned_videos_rejects_copy_size_mismatch_and_cleans_prepared_files() {
        let dir = tempdir().unwrap();
        let planned = planned_for_apply(dir.path(), "ABC-123");

        let report = apply_planned_videos(
            std::slice::from_ref(&planned),
            &FakeDownloader { fail_url: None },
            &FakeMover {
                rename_fails: true,
                copied_len: Some(1),
                delete_fails: false,
            },
            false,
        );

        assert_eq!(report.move_failures, vec![planned.source_path.clone()]);
        assert!(planned.source_path.exists());
        assert!(!planned.target_video_path.exists());
        assert!(!planned.nfo_path.exists());
        assert!(!planned.work_dir.exists());
    }

    #[test]
    fn test_apply_planned_videos_fail_fast_stops_after_first_failure() {
        let dir = tempdir().unwrap();
        let first = planned_for_apply(dir.path(), "BAD-001");
        let second = planned_for_apply(dir.path(), "OK-002");
        let mut first = first;
        first.metadata.title = String::new();

        let report = apply_planned_videos(
            &[first.clone(), second.clone()],
            &FakeDownloader { fail_url: None },
            &FakeMover {
                rename_fails: false,
                copied_len: None,
                delete_fails: false,
            },
            true,
        );

        assert_eq!(report.nfo_failures, vec![first.source_path.clone()]);
        assert!(second.source_path.exists());
        assert!(!second.target_video_path.exists());
    }

    #[test]
    fn test_write_planned_nfos_writes_same_basename_nfo() {
        let dir = tempdir().unwrap();
        let work_dir = dir.path().join("#演员").join("[2024] ABC-123 - 标题");
        let nfo_path = work_dir.join("ABC-123.nfo");
        let planned = PlannedVideo {
            source_path: dir.path().join("ABC-123.mp4"),
            target_video_path: work_dir.join("ABC-123.mp4"),
            nfo_path: nfo_path.clone(),
            poster_path: None,
            thumb_path: None,
            fanart_path: None,
            extrafanart_paths: Vec::new(),
            work_dir,
            actor_dir: dir.path().join("#演员"),
            metadata: VideoMetadata {
                product_id: "ABC-123".to_string(),
                title: "标题".to_string(),
                ..Default::default()
            },
        };

        let written = write_planned_nfos(&[planned]).unwrap();

        assert_eq!(written, vec![nfo_path.clone()]);
        assert!(nfo_path.exists());
        assert!(fs::read_to_string(nfo_path)
            .unwrap()
            .contains("ABC-123 - 标题"));
    }

    #[test]
    fn test_convert_smb_url_to_unc_basic() {
        let result = convert_smb_url_to_unc("smb://host/share");
        assert_eq!(result.unwrap(), "\\\\host\\share");
    }

    #[test]
    fn test_convert_smb_url_to_unc_with_subdirectory() {
        let result = convert_smb_url_to_unc("smb://host/share/path");
        assert_eq!(result.unwrap(), "\\\\host\\share\\path");
    }

    #[test]
    fn test_convert_smb_url_to_unc_with_auth() {
        let result = convert_smb_url_to_unc("smb://user:pass@host/share");
        assert_eq!(result.unwrap(), "\\\\host\\share");
    }

    #[test]
    fn test_convert_smb_url_to_unc_with_trailing_slash() {
        let result = convert_smb_url_to_unc("smb://host/share/");
        assert_eq!(result.unwrap(), "\\\\host\\share");
    }

    #[test]
    fn test_convert_smb_url_to_unc_root_share() {
        let result = convert_smb_url_to_unc("smb://host/");
        assert_eq!(result.unwrap(), "\\\\host");
    }

    #[test]
    fn test_convert_smb_url_to_unc_host_only() {
        let result = convert_smb_url_to_unc("smb://host");
        assert_eq!(result.unwrap(), "\\\\host");
    }

    #[test]
    fn test_convert_smb_url_to_unc_invalid_url() {
        let result = convert_smb_url_to_unc("smb://invalid url");
        assert!(result.is_err());
    }

    #[test]
    fn test_convert_smb_url_to_unc_non_smb() {
        let result = convert_smb_url_to_unc("http://host/share");
        assert!(result.is_err());
    }

    #[test]
    fn test_is_video_file_mp4() {
        assert!(is_video_file("test.mp4"));
    }

    #[test]
    fn test_is_video_file_mp4_uppercase() {
        assert!(is_video_file("test.MP4"));
    }

    #[test]
    fn test_is_video_file_mkv() {
        assert!(is_video_file("test.mkv"));
    }

    #[test]
    fn test_is_video_file_mkv_uppercase() {
        assert!(is_video_file("test.MKV"));
    }

    #[test]
    fn test_is_video_file_wmv() {
        assert!(is_video_file("test.wmv"));
    }

    #[test]
    fn test_is_video_file_wmv_uppercase() {
        assert!(is_video_file("test.WMV"));
    }

    #[test]
    fn test_is_video_file_with_path() {
        assert!(is_video_file("/path/to/test.mp4"));
    }

    #[test]
    fn test_is_video_file_extended_extensions() {
        for filename in ["test.avi", "test.mov", "test.m4v", "test.ts"] {
            assert!(
                is_video_file(filename),
                "{} should be a video file",
                filename
            );
        }
    }

    #[test]
    fn test_is_video_file_extended_extensions_uppercase() {
        for filename in ["test.AVI", "test.MOV", "test.M4V", "test.TS"] {
            assert!(
                is_video_file(filename),
                "{} should be a video file",
                filename
            );
        }
    }

    #[test]
    fn test_is_video_file_not_video() {
        assert!(!is_video_file("test.flv"));
    }

    #[test]
    fn test_is_video_file_txt() {
        assert!(!is_video_file("test.txt"));
    }

    #[test]
    fn test_is_image_file_jpg() {
        assert!(is_image_file("test.jpg"));
        assert!(is_image_file("test.JPEG"));
    }

    #[test]
    fn test_is_image_file_png() {
        assert!(is_image_file("test.png"));
    }

    #[test]
    fn test_is_image_file_not_image() {
        assert!(!is_image_file("test.mp4"));
        assert!(!is_image_file("test.txt"));
    }

    #[test]
    fn test_extract_id_standard_format() {
        assert_eq!(
            extract_id_from_filename("ABC-123.mp4"),
            Some("ABC-123".to_string())
        );
    }

    #[test]
    fn test_extract_id_another_standard() {
        assert_eq!(
            extract_id_from_filename("XYZ-456.mkv"),
            Some("XYZ-456".to_string())
        );
    }

    #[test]
    fn test_extract_id_no_dash() {
        assert_eq!(
            extract_id_from_filename("XYZ456.mp4"),
            Some("XYZ456".to_string())
        );
    }

    #[test]
    fn test_extract_id_another_no_dash() {
        assert_eq!(
            extract_id_from_filename("ABC789.mkv"),
            Some("ABC789".to_string())
        );
    }

    #[test]
    fn test_extract_id_with_path() {
        assert_eq!(
            extract_id_from_filename("/path/to/ABC-123.mp4"),
            Some("ABC-123".to_string())
        );
    }

    #[test]
    fn test_extract_id_long_prefix() {
        assert_eq!(
            extract_id_from_filename("ABCD-1234.wmv"),
            Some("ABCD-1234".to_string())
        );
    }

    #[test]
    fn test_extract_id_long_suffix() {
        assert_eq!(
            extract_id_from_filename("AB-12345.mp4"),
            Some("AB-12345".to_string())
        );
    }

    #[test]
    fn test_extract_id_no_valid_id() {
        assert_eq!(extract_id_from_filename("video.mp4"), None);
    }

    #[test]
    fn test_extract_id_only_numbers() {
        assert_eq!(extract_id_from_filename("123-456.mp4"), None);
    }

    #[test]
    fn test_extract_id_with_special_chars() {
        assert_eq!(
            extract_id_from_filename("ABC-123_test.mp4"),
            Some("ABC-123".to_string())
        );
    }

    #[test]
    fn test_extract_id_ignores_download_site_prefix() {
        assert_eq!(
            extract_id_from_filename("hhd800.com@MIDA-307.mp4"),
            Some("MIDA-307".to_string())
        );
        assert_eq!(
            extract_id_from_filename("hhd800.com@ABC123.mp4"),
            Some("ABC123".to_string())
        );
    }

    #[test]
    fn test_extract_id_prefers_dashed_id_over_earlier_site_token() {
        assert_eq!(
            extract_id_from_filename("hhd800.com MIDA-307.mp4"),
            Some("MIDA-307".to_string())
        );
    }

    #[test]
    fn test_extract_id_after_numeric_site_prefix() {
        assert_eq!(
            extract_id_from_filename("4k2.com@13dsvr01794_2_8k.mp4"),
            Some("dsvr01794".to_string())
        );
    }

    #[test]
    fn test_extract_id_multiple_possible() {
        assert_eq!(
            extract_id_from_filename("ABC-123_DEF-456.mp4"),
            Some("ABC-123".to_string())
        );
    }

    #[test]
    fn test_extract_id_uppercase_letters() {
        assert_eq!(
            extract_id_from_filename("ABCDEF-123456.mp4"),
            Some("ABCDEF-123456".to_string())
        );
    }

    #[test]
    fn test_extract_id_lowercase_letters() {
        assert_eq!(
            extract_id_from_filename("abc-123.mp4"),
            Some("abc-123".to_string())
        );
    }

    #[test]
    fn test_extract_id_mixed_case() {
        assert_eq!(
            extract_id_from_filename("AbC-123.mp4"),
            Some("AbC-123".to_string())
        );
    }

    #[test]
    fn test_extract_id_no_extension() {
        assert_eq!(
            extract_id_from_filename("ABC-123"),
            Some("ABC-123".to_string())
        );
    }

    #[test]
    fn test_extract_id_with_alphanumeric_prefix() {
        assert_eq!(
            extract_id_from_filename("T38-052.mp4"),
            Some("T38-052".to_string())
        );
    }

    #[test]
    fn test_extract_id_with_suffixes() {
        assert_eq!(
            extract_id_from_filename("XYZ-123-C.mp4"),
            Some("XYZ-123".to_string())
        );
        assert_eq!(
            extract_id_from_filename("XYZ-123-中文字符串-C.mkv"),
            Some("XYZ-123".to_string())
        );
        assert_eq!(
            extract_id_from_filename("XYZ-123-中文字符串.mp4"),
            Some("XYZ-123".to_string())
        );
        assert_eq!(
            extract_id_from_filename("XYZ-123-CD1.mp4"),
            Some("XYZ-123".to_string())
        );
    }

    #[test]
    fn test_extract_id_empty_string() {
        assert_eq!(extract_id_from_filename(""), None);
    }

    #[test]
    fn test_extract_prefix_basic() {
        assert_eq!(extract_prefix_from_id("ABC-123"), Some("ABC".to_string()));
        assert_eq!(extract_prefix_from_id("mida-983"), Some("mida".to_string()));
        assert_eq!(extract_prefix_from_id("star-123"), Some("star".to_string()));
    }

    #[test]
    fn test_extract_prefix_no_dash() {
        assert_eq!(extract_prefix_from_id("XYZ456"), Some("XYZ".to_string()));
    }

    #[test]
    fn test_extract_prefix_invalid() {
        assert_eq!(extract_prefix_from_id("123-456"), None);
        assert_eq!(extract_prefix_from_id(""), None);
    }

    #[test]
    fn test_extract_video_part_with_resolution_suffix() {
        assert_eq!(
            extract_video_part_from_filename("4k2.com@13dsvr01794_1_8k.mp4"),
            Some("1".to_string())
        );
        assert_eq!(
            extract_video_part_from_filename("twojav.com@urvrsp00535_2_8k.mp4"),
            Some("2".to_string())
        );
    }

    #[test]
    fn test_extract_video_part_cd_suffix() {
        assert_eq!(
            extract_video_part_from_filename("XYZ-123-CD1.mp4"),
            Some("1".to_string())
        );
    }

    #[test]
    fn test_extended_video_extensions_strip_extension_for_id_and_part() {
        assert_eq!(
            extract_id_from_filename("ABC-123.mov"),
            Some("ABC-123".to_string())
        );
        assert_eq!(
            extract_video_part_from_filename("XYZ-123-CD2.ts"),
            Some("2".to_string())
        );
    }

    #[test]
    fn test_extract_video_part_none_for_plain_id() {
        assert_eq!(extract_video_part_from_filename("START-476.mp4"), None);
        assert_eq!(extract_video_part_from_filename("T38-052.mp4"), None);
    }

    #[test]
    fn test_is_distinct_video_part() {
        assert!(is_distinct_video_part(
            "4k2.com@13dsvr01794_1_8k.mp4",
            "4k2.com@13dsvr01794_2_8k.mp4"
        ));
        assert!(!is_distinct_video_part(
            "4k2.com@13dsvr01794_1_8k.mp4",
            "other@13dsvr01794_1_8k.mp4"
        ));
        assert!(!is_distinct_video_part(
            "START-476.mp4",
            "hhd800.com@START-476.mp4"
        ));
    }
}
