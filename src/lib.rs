use regex::Regex;
use std::sync::OnceLock;
use url::Url;

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

pub fn is_video_file(filename: &str) -> bool {
    let lower = filename.to_lowercase();
    lower.ends_with(".mp4") || lower.ends_with(".mkv") || lower.ends_with(".wmv")
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

    let re_video = RE_VIDEO.get_or_init(|| Regex::new(r".*\.(?i)(mp4|mkv|wmv)$").unwrap());
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

    let re_video = RE_VIDEO.get_or_init(|| Regex::new(r".*\.(?i)(mp4|mkv|wmv)$").unwrap());
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
    fn test_is_video_file_not_video() {
        assert!(!is_video_file("test.avi"));
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
