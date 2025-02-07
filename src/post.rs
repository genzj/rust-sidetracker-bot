use atrium_api::app::bsky::embed::record::ViewRecordRefs;
use atrium_api::app::bsky::feed::defs::{PostView, ThreadViewPost};
use atrium_api::app::bsky::feed::defs::{PostViewEmbedRefs, ThreadViewPostParentRefs};
use atrium_api::app::bsky::feed::post::RecordData;
use atrium_api::types::string::{Cid, Did};
use atrium_api::types::{TryFromUnknown, Union, Unknown};
use log::debug;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
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
    pub cid: Cid,
    pub did: Did,
    pub handle: String,
    pub text: String,
    pub uri: String,
    pub idx: u32,
}

impl Post {
    pub fn new(
        cid: Cid,
        did: Did,
        handle: impl Into<String>,
        text: impl Into<String>,
        uri: impl Into<String>,
        idx: u32,
    ) -> Self {
        Self {
            cid,
            handle: handle.into(),
            did,
            text: text.into(),
            uri: uri.into(),
            idx,
        }
    }

    pub fn get_share_uri(&self) -> String {
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
                    box_record.cid.clone(),
                    box_record.author.did.clone(),
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

pub(crate) struct FlattenedThread {
    /// the root of this thread. Note: it's not necessarily the first post of the posts queue when
    /// the root post contains an embedded post
    pub root: Rc<RefCell<Post>>,
    /// the leaf post that leads to this thread.
    pub entrance: Rc<RefCell<Post>>,
    /// the posts in this thread, from the earliest to the latest
    pub posts: VecDeque<Rc<RefCell<Post>>>,
}

impl From<&ThreadViewPost> for FlattenedThread {
    fn from(value: &ThreadViewPost) -> Self {
        let mut result = VecDeque::with_capacity(10);
        let mut cur: &ThreadViewPost = value;
        let mut entrance: Option<Rc<RefCell<Post>>> = None;
        let root: Option<Rc<RefCell<Post>>>;
        loop {
            let post = &cur.post;
            let post = Post::new(
                post.cid.clone(),
                post.author.did.clone(),
                post.author.handle.to_string(),
                parse_post_text(post),
                parse_post_uri(post),
                0,
            );
            // ignore non text posts
            if post.text.len() > 0 {
                result.push_front(Rc::new(RefCell::from(post)));
            }
            entrance.get_or_insert_with(|| result.front().unwrap().clone());
            if cur.parent.is_none() {
                root = result.front().map(|p| p.clone());
                if let Some(post) = parse_embedded(&cur.post.embed) {
                    result.push_front(Rc::new(RefCell::from(post)));
                }
                break;
            } else if let Some(k) = get_parent(cur) {
                cur = k;
            }
        }

        // renumber the posts
        let mut idx: u32 = 1;
        for p in result.iter_mut() {
            p.borrow_mut().idx = idx;
            debug!("{:?} {}", p, p.borrow().get_share_uri());
            idx += 1;
        }

        Self {
            root: root.unwrap(),
            entrance: entrance.unwrap(),
            posts: result,
        }
    }
}

impl From<&FlattenedThread> for VecDeque<Post> {
    fn from(value: &FlattenedThread) -> Self {
        VecDeque::<Post>::from_iter(
            value.posts
                .iter()
                .map(|p| p.borrow().clone()),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use crate::post::tests::TestPost::{LeafPostThread, RootPostThread};
    use atrium_api::app::bsky::feed::get_post_thread;

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
        assert_eq!(
            embedded.cid,
            Cid::from_str("bafyreihvgtbjqmyo2ocpfic3rgjtvepbopcfhsqwxynl2shc4cww3nnjly").unwrap()
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
    fn test_flattened_thread() {
        let thread = load_test_thread(LeafPostThread);
        let flattened = FlattenedThread::from(&thread);
        assert_eq!(flattened.posts.len(), 13);
        assert_eq!(
            flattened.root.borrow().uri,
            "at://did:plc:xn5b64qpivpq55wumwf6wdjg/app.bsky.feed.post/3le7txyg4y22e"
        );
        assert_eq!(
            flattened.root.borrow().idx,
            2
        );
        assert_eq!(
            flattened.entrance.borrow().uri,
            "at://did:plc:xn5b64qpivpq55wumwf6wdjg/app.bsky.feed.post/3leb44umzuc2l"
        );
        assert_eq!(
            flattened.entrance.borrow().idx,
            13
        );
    }
}
