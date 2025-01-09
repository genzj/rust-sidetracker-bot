use atrium_api::app::bsky::embed::record::ViewRecordRefs;
use atrium_api::app::bsky::feed::defs::{PostView, ThreadViewPost};
use atrium_api::app::bsky::feed::defs::{PostViewEmbedRefs, ThreadViewPostParentRefs};
use atrium_api::app::bsky::feed::post::RecordData;
use atrium_api::types::{TryFromUnknown, Union, Unknown};
use log::debug;
use std::collections::VecDeque;
use url::Url;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PostLocator {
    repository: String,
    rkey: String,
}

impl PostLocator {
    pub fn new(repository: impl Into<String>, rkey: impl Into<String>) -> Self {
        Self {
            repository: repository.into(),
            rkey: rkey.into(),
        }
    }

    pub fn from_url(url: &str) -> Result<Self, url::ParseError> {
        // workaround of url parsing treating : as the port separator
        let use_did = url.contains("did:plc:");
        let url = url.replace("did:plc:", "did_plc_");
        let url = Url::parse(&url)?;
        match url.scheme() {
            "at" => {
                let paths: Vec<&str> = url.path_segments().unwrap().collect();
                if paths.len() != 2 {
                    return Err(url::ParseError::InvalidDomainCharacter);
                }
                let repo = if use_did {
                    url.host_str().unwrap().replace("did_plc_", "did:plc:")
                } else {
                    url.host_str().unwrap().to_string()
                };
                Ok(Self::new(&repo, *paths.last().unwrap()))
            }
            "http" | "https" => {
                let paths: Vec<&str> = url.path_segments().unwrap().collect();
                if paths.len() != 4 {
                    return Err(url::ParseError::InvalidDomainCharacter);
                }
                let repo = if use_did {
                    paths[1].replace("did_plc_", "did:plc:")
                } else {
                    paths[1].to_string()
                };
                Ok(Self::new(&repo, *paths.last().unwrap()))
            }
            _ => Err(url::ParseError::InvalidDomainCharacter),
        }
    }

    pub fn at_uri(&self) -> String {
        format!("at://{}/app.bsky.feed.post/{}", self.repository, self.rkey)
    }

    pub fn app_uri(&self) -> String {
        format!(
            "https://bsky.app/profile/{}/post/{}",
            self.repository, self.rkey
        )
    }
}

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
        PostLocator::from_url(&self.uri).unwrap().app_uri()
    }
}

pub fn parse_record_from_unknown(unknown: &Unknown) -> Option<RecordData> {
    if let Ok(record) = TryFromUnknown::try_from_unknown(unknown.clone()) {
        Some(record)
    } else {
        None
    }
}

pub fn parse_post_text(post: &PostView) -> String {
    if let Some(record) = parse_record_from_unknown(&post.record) {
        record.text.trim().to_string()
    } else {
        String::new()
    }
}

pub fn parse_post_uri(post: &PostView) -> String {
    post.uri.clone()
}

pub fn parse_post_author_handle(post: &PostView) -> String {
    post.author.handle.to_string()
}

pub fn get_parent<'a>(thread: &'a ThreadViewPost) -> Option<&'a ThreadViewPost> {
    if let Some(Union::Refs(ThreadViewPostParentRefs::ThreadViewPost(k))) = &thread.parent {
        Some(k)
    } else {
        None
    }
}

pub fn parse_embedded(post: &Option<Union<PostViewEmbedRefs>>) -> Option<Post> {
    if let &Some(Union::Refs(PostViewEmbedRefs::AppBskyEmbedRecordView(ref box_view))) = post {
        if let &Union::Refs(ViewRecordRefs::ViewRecord(ref box_record)) = &box_view.record {
            if let Some(record) = parse_record_from_unknown(&box_record.value) {
                return Some(Post::new(
                    box_record.author.handle.as_str(),
                    record.text,
                    box_record.uri.as_str(),
                    0,
                ));
            }
        }
    }
    None
}

pub fn flatten_thread(thread_view_post: &ThreadViewPost) -> VecDeque<Post> {
    let mut result = VecDeque::with_capacity(10);
    let mut cur: &ThreadViewPost = thread_view_post;
    loop {
        let post = &cur.post;
        let post = Post::new(
            parse_post_author_handle(post),
            parse_post_text(post),
            parse_post_uri(post),
            0,
        );
        // ignore non text posts
        if post.text.len() > 0 {
            result.push_front(post)
        }
        if cur.parent.is_none() {
            if let Some(post) = parse_embedded(&cur.post.embed) {
                result.push_front(post);
            }
            break;
        } else if let Some(k) = get_parent(cur) {
            cur = k;
        }
    }

    // renumber the posts
    let mut idx: u32 = 1;
    for p in result.iter_mut() {
        p.idx = idx;
        debug!("{:?} {}", p, p.get_share_uri());
        idx += 1;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use atrium_api::app::bsky::feed::get_post_thread;
    use crate::post::tests::TestPost::{LeafPostThread, RootPostThread};

    enum TestPost {
        LeafPostThread,
        RootPostThread,
    }

    fn load_test_thread(test_post: TestPost) -> ThreadViewPost {
        let test_file = match test_post {
            TestPost::LeafPostThread => "test_data/thread_3leb44umzuc2l.json5",
            TestPost::RootPostThread => "test_data/thread_3lfd7fhrkyk24.json5",
        };
        let output: get_post_thread::Output =
            serde_json5::from_slice(&std::fs::read(test_file).unwrap()).unwrap();
        if let Union::Refs(get_post_thread::OutputThreadRefs::AppBskyFeedDefsThreadViewPost(post)) =
            &output.thread
        {
            *post.to_owned()
        } else {
            panic!("unexpected test data");
        }
    }

    #[test]
    fn test_parse_post_text() {
        let thread = load_test_thread(LeafPostThread);
        let text = parse_post_text(&thread.post);
        assert_eq!(text, "猛吃！");
    }

    #[test]
    fn test_parse_post_uri() {
        let thread = load_test_thread(LeafPostThread);
        let uri = parse_post_uri(&thread.post);
        assert_eq!(
            uri,
            "at://did:plc:xn5b64qpivpq55wumwf6wdjg/app.bsky.feed.post/3leb44umzuc2l"
        );
    }

    #[test]
    fn test_parse_post_author_handle() {
        let thread = load_test_thread(LeafPostThread);
        let handle = parse_post_author_handle(&thread.post);
        assert_eq!(handle, "nghua.me");
    }

    #[test]
    fn test_get_parent() {
        let thread = load_test_thread(LeafPostThread);
        let parent = get_parent(&thread);
        assert!(parent.is_some());
        let uri = parse_post_uri(&parent.unwrap().post);
        assert_eq!(
            uri,
            "at://did:plc:7tf4afounuzjqioojiwln3jv/app.bsky.feed.post/3leb3s4oc222d"
        );
    }

    #[test]
    fn test_get_parent_return_none() {
        let thread = load_test_thread(RootPostThread);
        let parent = get_parent(&thread);
        assert!(parent.is_none());
    }

    #[test]
    fn test_parse_embedded() {
        let thread = load_test_thread(LeafPostThread);
        let mut level = Some(&thread);
        while let cur @ Some(_) = get_parent(level.unwrap()) {
            level = cur;
        }

        let embedded = parse_embedded(&level.unwrap().post.embed).unwrap();
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
        let thread = load_test_thread(LeafPostThread);
        let embedded = parse_embedded(&thread.post.embed);
        assert!(embedded.is_none());
    }

    #[test]
    fn test_flatten_thread() {
        let thread = load_test_thread(LeafPostThread);
        let flattened = flatten_thread(&thread);
        assert_eq!(flattened.len(), 13);
        assert_eq!(
            flattened[0].uri,
            "at://did:plc:fkjudld5cg4ailkuyec65wvg/app.bsky.feed.post/3le73kidz7k2e"
        );
        assert_eq!(
            flattened[1].uri,
            "at://did:plc:xn5b64qpivpq55wumwf6wdjg/app.bsky.feed.post/3le7txyg4y22e"
        );
        assert_eq!(
            flattened.back().unwrap().uri,
            "at://did:plc:xn5b64qpivpq55wumwf6wdjg/app.bsky.feed.post/3leb44umzuc2l"
        );
    }
}
