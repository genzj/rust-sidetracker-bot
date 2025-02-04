use atrium_api::{
    app::bsky::feed::post::{Record, RecordData, ReplyRef, ReplyRefData},
    com::atproto::repo::strong_ref,
    types::string::Datetime,
};
use ellipse::Ellipse;

use crate::post::Post;

#[derive(Debug, PartialEq)]
pub(crate) struct SideTracker {
    /// The sidetracking post
    post: Option<Post>,
    /// The root post of the thread
    root: Post,
    /// The leaf post from where this checking is triggered
    entrance: Post,
}

impl SideTracker {
    pub(crate) fn new(post: Option<Post>, root: Post, entrance: Post) -> SideTracker {
        SideTracker {
            post,
            root,
            entrance,
        }
    }

    pub(crate) fn build_reply(&self) -> Record {
        let text = if let Some(ref p) = self.post {
            format!(
                "最有可能的歪楼犯： @{}\n罪证： {}\n现场还原： {}",
                p.handle,
                p.text.as_str().truncate_ellipse(20),
                p.get_share_uri()
            )
        } else {
            "太好了，没有找到歪楼犯".to_string()
        };

        // TODO handle post link and user handler.
        //   ref: https://docs.bsky.app/docs/advanced-guides/posts#mentions-and-links
        Record::from(RecordData {
            created_at: Datetime::now(),
            entities: None,
            facets: None,
            labels: None,
            langs: None,
            reply: Some(ReplyRef::from(Into::<ReplyRefData>::into(self))),
            tags: None,
            text,
            embed: None,
        })
    }
}

impl From<&Post> for strong_ref::MainData {
    fn from(value: &Post) -> Self {
        Self {
            cid: value.cid.to_owned(),
            uri: value.uri.to_owned(),
        }
    }
}

impl From<&Post> for strong_ref::Main {
    fn from(value: &Post) -> Self {
        strong_ref::Main::from(strong_ref::MainData::from(value))
    }
}

impl From<&SideTracker> for ReplyRefData {
    fn from(value: &SideTracker) -> Self {
        Self {
            parent: (&value.entrance).into(),
            root: (&value.root).into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use atrium_api::types::string::Cid;
    use std::str::FromStr;

    #[test]
    fn test_side_tracker() {
        let root = Post::new(
            Cid::from_str("bafyreihvgtbjqmyo2ocpfic3rgjtvepbopaaaaawcccccsxxxxxw3nnjly").unwrap(),
            "handle1".to_string(),
            "text_root".to_string(),
            "at://did:plc:test/app.bsky.feed.post/root".to_string(),
            1,
        );
        let entrance = Post::new(
            Cid::from_str("bafyreihvgtbjqmyo2ocpfic3rgjtvepbopbbbbbwaaaaasyyyyyw3nnjly").unwrap(),
            "handle2".to_string(),
            "text_entrance".to_string(),
            "at://did:plc:test/app.bsky.feed.post/entrance".to_string(),
            12,
        );
        let post = Post::new(
            Cid::from_str("bafyreihvgtbjqmyo2ocpfic3rgjtvepbopbbbbbwaaaaaszzzzzw3nnjly").unwrap(),
            "handle3".to_string(),
            "text_post".to_string(),
            "at://did:plc:test/app.bsky.feed.post/post".to_string(),
            6,
        );
        let side_tracker = SideTracker::new(Some(post), root, entrance);
        let reply = side_tracker.build_reply();
        assert_eq!(reply.text, "最有可能的歪楼犯： @handle3\n罪证： text_post\n现场还原： https://bsky.app/profile/did:plc:test/post/post");
    }

    #[test]
    fn test_empty_side_tracker() {
        let root = Post::new(
            Cid::from_str("bafyreihvgtbjqmyo2ocpfic3rgjtvepbopaaaaawcccccsxxxxxw3nnjly").unwrap(),
            "handle".to_string(),
            "text".to_string(),
            "at://did:plc:test/app.bsky.feed.post/root".to_string(),
            1,
        );
        let entrance = Post::new(
            Cid::from_str("bafyreihvgtbjqmyo2ocpfic3rgjtvepbopbbbbbwaaaaasyyyyyw3nnjly").unwrap(),
            "handle".to_string(),
            "text".to_string(),
            "at://did:plc:test/app.bsky.feed.post/entrance".to_string(),
            12,
        );
        let side_tracker = SideTracker::new(None, root, entrance);
        let reply = side_tracker.build_reply();
        assert_eq!(reply.text, "太好了，没有找到歪楼犯");
    }
}
