use atrium_api::app::bsky::richtext::facet;
use atrium_api::app::bsky::richtext::facet::{
    ByteSlice, ByteSliceData, MainFeaturesItem, Mention, MentionData,
};
use atrium_api::types::string::Language;
use atrium_api::types::Union;
use atrium_api::{
    app::bsky::feed::post::{RecordData, ReplyRef, ReplyRefData},
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

    pub(crate) fn build_reply(&self) -> RecordData {
        let mut facets: Vec<facet::Main> = Vec::new();
        let text = if let Some(ref p) = self.post {
            let mut text = "最有可能的歪楼犯：".to_string();
            {
                let mention_start = text.len();
                text.push_str("@");
                text.push_str(&p.handle);
                let mention_end = text.len();
                text.push_str("\n");
                let mention = MainFeaturesItem::Mention(Box::from(Mention::from(MentionData {
                    did: p.did.clone(),
                })));
                facets.push(facet::Main::from(facet::MainData {
                    features: vec![Union::Refs(mention)],
                    index: ByteSlice::from(ByteSliceData {
                        byte_start: mention_start,
                        byte_end: mention_end,
                    }),
                }));
            }

            text.push_str(format!("罪证：{}\n", p.text.as_str().truncate_ellipse(20)).as_str());

            {
                let link_start = text.len();
                text.push_str(p.get_share_uri().as_str());
                let link_end = text.len();
                let link = MainFeaturesItem::Link(Box::from(facet::Link::from(facet::LinkData {
                    uri: p.get_share_uri(),
                })));
                facets.push(facet::Main::from(facet::MainData {
                    features: vec![Union::Refs(link)],
                    index: ByteSlice::from(ByteSliceData {
                        byte_start: link_start,
                        byte_end: link_end,
                    }),
                }));
            }
            text
        } else {
            "太好了，没有找到歪楼犯".to_string()
        };

        RecordData {
            created_at: Datetime::now(),
            entities: None,
            facets: if facets.len() > 0 { Some(facets) } else { None },
            labels: None,
            langs: Some(vec![
                Language::new("zh-CN".to_string()).unwrap(),
                Language::new("en-US".to_string()).unwrap(),
            ]),
            reply: Some(ReplyRef::from(Into::<ReplyRefData>::into(self))),
            tags: None,
            text,
            embed: None,
        }
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
    use atrium_api::types::string::{Cid, Did};
    use std::str::FromStr;

    #[test]
    fn test_side_tracker() {
        let root = Post::new(
            Cid::from_str("bafyreihvgtbjqmyo2ocpfic3rgjtvepbopaaaaawcccccsxxxxxw3nnjly").unwrap(),
            Did::from_str("did:plc:fkjudld5cgxxxxxxxxxxxxxx").unwrap(),
            "handle1".to_string(),
            "text_root".to_string(),
            "at://did:plc:test/app.bsky.feed.post/root".to_string(),
            1,
        );
        let entrance = Post::new(
            Cid::from_str("bafyreihvgtbjqmyo2ocpfic3rgjtvepbopbbbbbwaaaaasyyyyyw3nnjly").unwrap(),
            Did::from_str("did:plc:fkjudld5cgyyyyyyyyyyyyyy").unwrap(),
            "handle2".to_string(),
            "text_entrance".to_string(),
            "at://did:plc:test/app.bsky.feed.post/entrance".to_string(),
            12,
        );
        let post = Post::new(
            Cid::from_str("bafyreihvgtbjqmyo2ocpfic3rgjtvepbopbbbbbwaaaaaszzzzzw3nnjly").unwrap(),
            Did::from_str("did:plc:fkjudld5cgzzzzzzzzzzzzzz").unwrap(),
            "handle3".to_string(),
            "text post but very very long".to_string(),
            "at://did:plc:test/app.bsky.feed.post/post".to_string(),
            6,
        );
        let side_tracker = SideTracker::new(Some(post), root, entrance);
        let reply = side_tracker.build_reply();
        assert_eq!(reply.text, "最有可能的歪楼犯：@handle3\n罪证：text post but very v...\nhttps://bsky.app/profile/did:plc:test/post/post");
        let mention = reply.facets.as_ref().unwrap().get(0).unwrap();
        assert_eq!(mention.index.byte_start, 27);
        assert_eq!(mention.index.byte_end, 35);
        let link = reply.facets.as_ref().unwrap().get(1).unwrap();
        assert_eq!(link.index.byte_start, 69);
        assert_eq!(link.index.byte_end, 116);
    }

    #[test]
    fn test_empty_side_tracker() {
        let root = Post::new(
            Cid::from_str("bafyreihvgtbjqmyo2ocpfic3rgjtvepbopaaaaawcccccsxxxxxw3nnjly").unwrap(),
            Did::from_str("did:plc:fkjudld5cgxxxxxxxxxxxxxx").unwrap(),
            "handle".to_string(),
            "text".to_string(),
            "at://did:plc:test/app.bsky.feed.post/root".to_string(),
            1,
        );
        let entrance = Post::new(
            Cid::from_str("bafyreihvgtbjqmyo2ocpfic3rgjtvepbopbbbbbwaaaaasyyyyyw3nnjly").unwrap(),
            Did::from_str("did:plc:fkjudld5cgyyyyyyyyyyyyyy").unwrap(),
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
