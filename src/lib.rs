use regex::Regex;
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
    let re_video = Regex::new(r".*\.(?i)(mp4|mkv|wmv)").unwrap();
    re_video.is_match(filename)
}

pub fn extract_id_from_filename(filename: &str) -> Option<String> {
    let re_video = Regex::new(r".*\.(?i)(mp4|mkv|wmv)$").unwrap();
    let re_id = Regex::new(r"[[:alpha:]]+-\d+|[[:alpha:]]+\d+").unwrap();

    let name_without_ext = if re_video.is_match(filename) {
        let pos = filename.rfind('.').unwrap();
        &filename[..pos]
    } else {
        filename
    };

    re_id.find(name_without_ext).map(|m| m.as_str().to_string())
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
}
