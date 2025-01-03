use atproto_api::{Agent, AtpAgent, Session};
use futures::FutureExt;
use log::{debug, info};
use serde_json::{json, Value};
use std::env;
use std::panic::AssertUnwindSafe;

pub async fn must_create_agent() -> Result<AtpAgent, Box<dyn std::error::Error>> {
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
