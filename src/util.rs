#[cfg(test)]
use pretty_env_logger::env_logger;
use std::path::Path;
use tokio::io::AsyncWriteExt;

pub fn find_and_parse_first_integer(input: String) -> Option<u32> {
    let mut num_str = String::new();
    let mut found_number = false;

    for c in input.chars() {
        if c.is_digit(10) {
            num_str.push(c);
            found_number = true;
        } else if found_number {
            break;
        }
    }

    if let Ok(num) = num_str.parse::<u32>() {
        Some(num)
    } else {
        None
    }
}

#[cfg(test)]
pub fn ensure_tailing_slash(s: &str) -> String {
    let mut s = s.to_owned();
    if !s.ends_with('/') {
        s.push('/')
    }
    s
}

pub async fn dump_to_private_file<T>(
    file_path: impl AsRef<Path>,
    data: &T,
) -> Result<(), Box<dyn std::error::Error>>
where
    T: ?Sized + serde::Serialize,
{
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(file_path)
        .await?;
    let json = serde_json::to_string_pretty(data)?;
    file.write_all(json.as_bytes()).await?;
    file.flush().await?;
    Ok(())
}

pub async fn load_from_file<T>(file_path: impl AsRef<Path>) -> Result<T, Box<dyn std::error::Error>>
where
    T: serde::de::DeserializeOwned,
{
    if !is_file_exists(file_path.as_ref()).await {
        return Err("File not found".into());
    }
    let session = tokio::fs::read(file_path).await?;
    let data: T = serde_json::from_reader(std::io::Cursor::new(session))?;
    Ok(data)
}

pub async fn is_file_exists(file_path: impl AsRef<Path>) -> bool {
    tokio::fs::metadata(file_path).await.is_ok()
}

pub async fn remove_file(file_path: impl AsRef<Path>) -> std::io::Result<()> {
    tokio::fs::remove_file(file_path).await
}

#[cfg(test)]
#[allow(dead_code)]
pub fn init_test_logger() {
    let _ = env_logger::builder()
        .is_test(true)
        .filter_level(log::LevelFilter::Trace)
        .try_init();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn test_simple_number() {
        assert_eq!(find_and_parse_first_integer("123".to_string()), Some(123));
    }

    #[test]
    fn test_number_with_text() {
        assert_eq!(
            find_and_parse_first_integer("abc123def".to_string()),
            Some(123)
        );
    }

    #[test]
    fn test_text_before_number() {
        assert_eq!(
            find_and_parse_first_integer("abc456".to_string()),
            Some(456)
        );
    }

    #[test]
    fn test_text_after_number() {
        assert_eq!(
            find_and_parse_first_integer("789xyz".to_string()),
            Some(789)
        );
    }

    #[test]
    fn test_multiple_numbers() {
        assert_eq!(
            find_and_parse_first_integer("123 456".to_string()),
            Some(123)
        );
    }

    #[test]
    fn test_empty_string() {
        assert_eq!(find_and_parse_first_integer("".to_string()), None);
    }

    #[test]
    fn test_no_numbers() {
        assert_eq!(find_and_parse_first_integer("abc".to_string()), None);
    }

    #[test]
    fn test_number_too_large() {
        // Testing a number larger than u32::MAX
        assert_eq!(
            find_and_parse_first_integer("9994294967296".to_string()),
            None
        );
    }

    #[test]
    fn test_special_characters() {
        assert_eq!(
            find_and_parse_first_integer("!@#123$%^".to_string()),
            Some(123)
        );
    }

    #[test]
    fn test_ensure_tailing_slash() {
        let s = "https://example.com";
        assert_eq!(ensure_tailing_slash(&s), "https://example.com/");

        let s = "https://example.com/";
        assert_eq!(ensure_tailing_slash(&s), "https://example.com/");
    }

    #[tokio::test]
    async fn test_dump_to_private_file_and_load() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let data = "test";
        let file_path = "test.txt";
        let file_path = tmp_dir.path().join(file_path);
        dump_to_private_file(&file_path, &data).await.unwrap();
        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "\"test\"");

        let loaded: String = load_from_file(&file_path).await.unwrap();
        assert_eq!(loaded, data);

        let metadata = tokio::fs::metadata(&file_path).await.unwrap();
        assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
        tmp_dir.close().unwrap();
    }

    #[tokio::test]
    async fn test_file_exists_and_removal() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let file_path = tmp_dir.path().join("test.txt");
        tokio::fs::File::create(&file_path).await.unwrap();
        assert!(is_file_exists(&file_path).await);

        remove_file(&file_path).await.unwrap();
        assert!(!is_file_exists(&file_path).await);

        let file_path = tmp_dir.path().join("wrong-file.txt");
        assert!(!is_file_exists(&file_path).await);

        tmp_dir.close().unwrap();
    }
}
