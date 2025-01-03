use pretty_env_logger::env_logger;

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

pub fn ensure_tailing_slash(s: &str) -> String {
    let mut s = s.to_owned();
    if !s.ends_with('/') {
        s.push('/')
    }
    s
}

#[cfg(test)]
pub fn init_test_logger() {
    let _ = env_logger::builder()
        .is_test(true)
        .filter_level(log::LevelFilter::Trace)
        .try_init();
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_enensure_tailing_slash() {
        let s = "https://example.com";
        assert_eq!(ensure_tailing_slash(&s), "https://example.com/");

        let s = "https://example.com/";
        assert_eq!(ensure_tailing_slash(&s), "https://example.com/");
    }
}
