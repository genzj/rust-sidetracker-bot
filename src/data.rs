use ellipse::Ellipse;
use atrium_api::{
    app::bsky::feed::post::{Record, RecordData, ReplyRef, ReplyRefData},
    com::atproto::repo::strong_ref,
    types::string::Datetime,
};

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

pub(crate) type CheckResult = Option<SideTracker>;

impl SideTracker {
    pub(crate) fn new(post: &Post, root: &Post, entrance: &Post) -> SideTracker {
        SideTracker {
            post: Some(post.clone()),
            root: root.clone(),
            entrance: entrance.clone(),
        }
    }

    pub(crate) fn not_found(root: &Post, entrance: &Post) -> SideTracker {
        SideTracker {
            post: None,
            root: root.clone(),
            entrance: entrance.clone(),
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
            text: text,
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
