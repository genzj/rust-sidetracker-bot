use atproto_api::{Agent, AtpAgent};
use serde_json::{json, Value};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Post {
    pub handle: String,
    pub text: String,
    pub uri: String,
    pub idx: u32,
}

impl Post {
    pub fn new(
        handle: impl Into<String>,
        text: impl Into<String>,
        uri: impl Into<String>,
        idx: u32,
    ) -> Self {
        Self {
            handle: handle.into(),
            text: text.into(),
            uri: uri.into(),
            idx,
        }
    }

    pub fn get_share_uri(&self) -> String {
        // https://bsky.app/profile/xijinpingoffical.bsky.social/post/3lelut5loqs2u
        self.uri
            .replacen("at://", "https://bsky.app/profile/", 1)
            .replacen("/app.bsky.feed.post/", "/post/", 1)
    }
}

pub async fn get_post_thread(
    agent: AtpAgent,
    uri: String,
) -> Result<Value, Box<dyn std::error::Error>> {
    let params = json!({
      "uri": uri,
      "depth": "1"
    });

    let res = agent
        .get("app.bsky.feed.getPostThread".to_string(), params)
        .await?;

    Ok(res)
}

pub fn parse_post_text(post: &Value) -> &str {
    post["record"]["text"].as_str().unwrap().trim()
}

pub fn parse_post_uri(post: &Value) -> &str {
    post["uri"].as_str().unwrap()
}

pub fn parse_post_author_handle(post: &Value) -> &str {
    post["author"]["handle"].as_str().unwrap()
}

pub fn parse_embedded(post: &Value) -> Option<Post> {
    let value = &post["post"]["embed"]["record"];
    let ret = match (
        value["value"]["text"].as_str(),
        value["author"]["handle"].as_str(),
        value["uri"].as_str(),
    ) {
        (Some(text), Some(handle), Some(uri)) if text.trim() != "" => {
            Some(Post::new(handle, text.trim(), uri, 0))
        }
        (_, _, _) => None,
    };
    ret
}

#[cfg(test)]
mod tests {
    use super::*;

    fn thread() -> Value {
        let data: Value =
            serde_json::from_str(include_str!("../test_data/thread_3leb44umzuc2l.json5")).unwrap();
        data["thread"].to_owned()
    }

    #[test]
    fn test_parse_post_text() {
        let thread = thread();
        let text = parse_post_text(&thread["post"]);
        assert_eq!(text, "猛吃！");
    }

    #[test]
    fn test_parse_post_uri() {
        let thread = thread();
        let uri = parse_post_uri(&thread["post"]);
        assert_eq!(
            uri,
            "at://did:plc:xn5b64qpivpq55wumwf6wdjg/app.bsky.feed.post/3leb44umzuc2l"
        );
    }

    #[test]
    fn test_parse_post_author_handle() {
        let thread = thread();
        let handle = parse_post_author_handle(&thread["post"]);
        assert_eq!(handle, "nghua.me");
    }

    #[test]
    fn test_parse_embedded() {
        //.thread.parent.parent.parent.parent.parent.parent.parent.parent.parent.parent.parent.parent
        let thread = thread();
        let mut level = &thread;
        loop {
            if let Value::Null = level["parent"] {
                break;
            }
            level = &level["parent"];
        }
        let embedded = parse_embedded(level).unwrap();
        assert_eq!(embedded.handle, "cotranedolphy.bsky.social");
        assert!(embedded.text.starts_with("以前有个同事"));
        assert_eq!(
            embedded.uri,
            "at://did:plc:fkjudld5cg4ailkuyec65wvg/app.bsky.feed.post/3le73kidz7k2e",
        );
        assert_eq!(embedded.idx, 0);
        assert_eq!(
            embedded.get_share_uri(),
            "https://bsky.app/profile/did:plc:fkjudld5cg4ailkuyec65wvg/post/3le73kidz7k2e"
        );
    }

    #[test]
    fn test_parse_embed_return_none() {
        let thread = thread();
        let embedded = parse_embedded(&thread);
        assert!(embedded.is_none());
    }
}
