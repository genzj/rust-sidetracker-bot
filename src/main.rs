mod openai;
mod post;
mod util;

use crate::openai::openai_locate_sidetracker;
use crate::post::{
    get_post_thread, parse_embedded, parse_post_author_handle, parse_post_text, parse_post_uri,
    Post,
};
use atproto_api::{Agent, AtpAgent, Session};
use dotenv::dotenv;
use futures::FutureExt;
use log::{debug, info};
use std::collections::VecDeque;
use std::env;
use std::panic::AssertUnwindSafe;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    pretty_env_logger::init();
    let agent = must_create_agent().await?;
    let res = get_post_thread(
        agent,
        "at://nghua.me/app.bsky.feed.post/3leb44umzuc2l".to_string(),
        // "at://demishuyan.bsky.social/app.bsky.feed.post/3lem7oosaz22t".to_string(),
    )
    .await?;
    let mut post = &res["thread"];
    let mut thread = VecDeque::with_capacity(2);
    loop {
        let p = Post::new(
            parse_post_author_handle(&post["post"]).to_string(),
            parse_post_text(&post["post"]).to_string(),
            parse_post_uri(&post["post"]).to_string(),
            0,
        );
        if p.text.len() > 0 {
            thread.push_front(p)
        }
        if post["parent"].is_null() {
            if let Some(p) = parse_embedded(&post) {
                thread.push_front(p)
            }
            break;
        }
        post = &post["parent"];
    }

    let mut idx: u32 = 1;
    for p in thread.iter_mut() {
        p.idx = idx;
        debug!("{:?} {}", p, p.get_share_uri());
        idx += 1;
    }

    if let Some(p) = openai_locate_sidetracker(&thread).await {
        println!(
            "最有可能的歪楼犯：{}\n罪证：{}\n现场还原：{}",
            p.handle,
            p.text,
            p.get_share_uri()
        );
    } else {
        println!("太好了，没有找到歪楼犯");
    }

    Ok(())
}

async fn must_create_agent() -> Result<AtpAgent, Box<dyn std::error::Error>> {
    let mut agent: Option<AtpAgent> = None;
    let session = env::var("BLUESKY_SESSION").unwrap_or("".to_string());
    if session.len() > 0 {
        debug!("loading from session {:?}", session);
        let mut session_agent = AtpAgent::new("https://bsky.social".to_string());
        match serde_json::from_str::<Session>(session.as_str()) {
            Ok(session) => {
                session_agent.session = Some(session);
                let refresh_result = AssertUnwindSafe(async {
                    debug!("trying refreshing session");
                    session_agent.clone().refresh_session().await
                })
                .catch_unwind()
                .await;
                match refresh_result {
                    Ok(Ok(())) => {
                        info!("session refreshed successfully");
                        agent = Some(session_agent)
                    }
                    Ok(Err(err)) => {
                        debug!("session refresh failed: {:?}", err);
                    }
                    Err(err) => {
                        debug!("session refresh panic: {:?}", err);
                    }
                }
            }
            Err(err) => {
                debug!("session load failed: {:?}", err);
            }
        }
    }

    if agent.is_none() {
        info!("resuming session failed, fallback to login");
        let login_agent = AtpAgent::new("https://bsky.social".to_string())
            .login(
                env::var("BLUESKY_IDENTIFIER").unwrap(),
                env::var("BLUESKY_PASSWORD").unwrap(),
            )
            .await?;
        debug!("{:?}", serde_json::to_string(&login_agent.session)?);
        agent = Some(login_agent);
    }
    Ok(agent.unwrap())
}
