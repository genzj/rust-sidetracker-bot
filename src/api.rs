use crate::util::{dump_to_private_file, is_file_exists, load_from_file};
use atproto_api::{Agent, AtpAgent};
use futures::FutureExt;
use log::{debug, info, trace};
use serde_json::{json, Value};
use std::env;
use std::panic::AssertUnwindSafe;

const SESSION_FILE: &str = "session.json";

pub async fn must_create_agent() -> Result<AtpAgent, Box<dyn std::error::Error>> {
    let mut agent: Option<AtpAgent> = None;
    if is_file_exists(SESSION_FILE).await {
        debug!("loading from session file {:?}", SESSION_FILE);
        let mut session_agent = AtpAgent::new("https://bsky.social".to_string());
        match load_from_file(SESSION_FILE).await {
            Ok(session) => {
                debug!("session file loaded successfully");
                session_agent.session = Some(session);
                let refresh_result = AssertUnwindSafe(async {
                    debug!("trying refreshing session");
                    session_agent.clone().refresh_session().await
                })
                    .catch_unwind()
                    .await;
                match refresh_result {
                    Ok(Ok(())) => {
                        info!("saved session refreshed successfully");
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
        info!("logged in successfully");
        trace!("logged in as {:?}", &login_agent.session);
        debug!("dumping session to file {SESSION_FILE}");
        dump_to_private_file(SESSION_FILE, &login_agent.session).await?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::ensure_tailing_slash;
    use atproto_api::Session;
    use mockito::Matcher::PartialJsonString;
    use mockito::{Matcher, Server};

    fn create_test_session() -> Session {
        let session = r#"{
            "accessJwt": "test-saved-access-jwt",
            "did": "test-did",
            "handle": "test-handle",
            "refreshJwt": "test-saved-refresh-jwt"
        }"#;
        serde_json::from_str::<Session>(session).unwrap()
    }

    fn create_test_agent(server: &Server) -> AtpAgent {
        let url = ensure_tailing_slash(&server.url());
        let mut agent = AtpAgent::new(url);
        agent.session = Some(create_test_session());
        agent
    }

    async fn mock_create_session(server: &mut Server) -> &mut Server {
        server
            .mock("POST", "/xrpc/com.atproto.server.createSession")
            .match_header("content-type", "application/json")
            .match_body(PartialJsonString(
                r#"{"identifier":"handle", "password":"password"}"#.to_string(),
            ))
            .match_header("User-Agent", "atproto_api/0.1.0")
            .with_status(200)
            .with_body(
                r#"{
                    "accessJwt": "test-logged-access-jwt",
                    "did": "test-did",
                    "handle": "test-handle",
                    "refreshJwt": "test-logged-refresh-jwt",
                }"#,
            )
            .create_async()
            .await;
        server
    }

    async fn mock_refresh_session(server: &mut Server) -> &mut Server {
        server
            .mock("POST", "/xrpc/com.atproto.server.refreshSession")
            .match_header(
                "authorization",
                Matcher::AnyOf(vec![
                    Matcher::Exact("Bearer test-logged-refresh-jwt".to_string()),
                    Matcher::Exact("Bearer test-saved-refresh-jwt".to_string()),
                ]),
            )
            .match_header("User-Agent", "atproto_api/0.1.0")
            .with_status(200)
            .with_body(
                r#"{
                    "accessJwt": "test-refreshed-access-jwt",
                    "did": "test-did",
                    "handle": "test-handle",
                    "refreshJwt": "test-refreshed-refresh-jwt",
                }"#,
            )
            .create_async()
            .await;
        server
    }

    const TEST_THREAD_URI: &'static str = "at://handle/app.bsky.feed.post/id";

    async fn mock_get_post_thread(server: &mut Server) -> &mut Server {
        server
            .mock("GET", "/xrpc/app.bsky.feed.getPostThread")
            .match_header("User-Agent", "atproto_api/0.1.0")
            .match_header(
                "authorization",
                Matcher::AnyOf(vec![
                    Matcher::Exact("Bearer test-logged-access-jwt".to_string()),
                    Matcher::Exact("Bearer test-saved-access-jwt".to_string()),
                    Matcher::Exact("Bearer test-refreshed-access-jwt".to_string()),
                ]),
            )
            .match_query(Matcher::AllOf(vec![
                Matcher::UrlEncoded("uri".to_string(), TEST_THREAD_URI.to_string()),
                Matcher::UrlEncoded("depth".to_string(), "1".to_string()),
            ]))
            .with_status(200)
            .with_body_from_file("test_data/thread_3leb44umzuc2l.json5")
            .create_async()
            .await;
        server
    }

    #[tokio::test]
    async fn test_get_post_thread() {
        let mut server = Server::new_async().await;
        mock_refresh_session(&mut server).await;
        mock_get_post_thread(&mut server).await;
        let agent = create_test_agent(&server);
        let res = get_post_thread(agent, TEST_THREAD_URI.to_string())
            .await
            .unwrap();
        assert!(res.is_object());
        assert_eq!(res["thread"]["$type"], "app.bsky.feed.defs#threadViewPost");
    }

    #[tokio::test]
    async fn test_agent_from_session() {}

    #[tokio::test]
    async fn test_agent_from_login() {
        let mut server = Server::new_async().await;
        mock_create_session(&mut server).await;
        mock_refresh_session(&mut server).await;
        mock_get_post_thread(&mut server).await;
    }
}
