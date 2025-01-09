use crate::util::{dump_to_private_file, is_file_exists, load_from_file};
use atrium_api::agent::{store::MemorySessionStore, AtpAgent, Session};
use atrium_api::app::bsky::feed::defs::ThreadViewPost;
use atrium_api::app::bsky::feed::get_post_thread::{OutputThreadRefs, ParametersData};
use atrium_api::types::Union;
use atrium_xrpc_client::reqwest::ReqwestClient;
use log::{info, trace};
use std::env;
use std::error::Error;
use std::ops::Deref;

const SESSION_FILE: &str = "session.json";

type BskyClient = AtpAgent<MemorySessionStore, ReqwestClient>;

fn new_client(base_url: &str) -> BskyClient {
    AtpAgent::new(ReqwestClient::new(base_url), MemorySessionStore::default())
}

pub async fn must_create_agent() -> Result<BskyClient, Box<dyn std::error::Error>> {
    let client = new_client("https://bsky.social");
    // TODO delegate to SessionStore
    if is_file_exists(SESSION_FILE).await {
        if let Ok(session) = load_from_file::<Session>(SESSION_FILE).await {
            if let Ok(_) = client.resume_session(session).await {
                return Ok(client);
            }
        }
    }
    client
        .login(
            env::var("BLUESKY_IDENTIFIER").unwrap(),
            env::var("BLUESKY_PASSWORD").unwrap(),
        )
        .await?;
    let _ = dump_to_private_file(SESSION_FILE, &client.get_session().await.unwrap()).await;
    Ok(client)
}

pub async fn get_post_thread(
    client: BskyClient,
    uri: String,
) -> Result<ThreadViewPost, Box<dyn Error>> {
    let res = client
        .api
        .app
        .bsky
        .feed
        .get_post_thread(
            ParametersData {
                depth: Some(1u16.try_into().unwrap()),
                parent_height: Some(20.try_into().unwrap()),
                uri: uri,
            }
            .into(),
        )
        .await?;

    match &res.thread {
        Union::Refs(OutputThreadRefs::AppBskyFeedDefsThreadViewPost(post)) => {
            trace!("downloaded post: {:?}", post);
            return Ok(post.deref().clone());
        }
        _ => {
            info!("post: {:?}", res);
        }
    }
    Err("not found".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use atrium_api::agent::Session;
    use mockito::Matcher::PartialJsonString;
    use mockito::{Matcher, Server};

    fn create_test_session() -> Session {
        let session = r#"{
            "accessJwt": "test-saved-access-jwt",
            "did": "did:plc:test_did",
            "handle": "test.handle",
            "refreshJwt": "test-saved-refresh-jwt"
        }"#;
        serde_json::from_str::<Session>(session).unwrap()
    }

    async fn create_test_agent(server: &Server) -> BskyClient {
        let url = &server.url();
        let url = url.strip_suffix('/').unwrap_or(url);
        let client = new_client(&url);
        let resume = client.resume_session(create_test_session()).await;
        info!("resume: {:?}", resume);
        assert!(resume.is_ok());
        client
    }

    async fn mock_create_session(server: &mut Server) -> &mut Server {
        server
            .mock("POST", "/xrpc/com.atproto.server.createSession")
            .match_header("content-type", "application/json")
            .match_body(PartialJsonString(
                r#"{"identifier":"handle", "password":"password"}"#.to_string(),
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                    "accessJwt": "test-logged-access-jwt",
                    "did": "did:plc:test_did",
                    "handle": "test.handle",
                    "refreshJwt": "test-logged-refresh-jwt",
                }"#,
            )
            .create_async()
            .await;
        server
    }

    async fn mock_get_session(server: &mut Server) -> &mut Server {
        server
            .mock("GET", "/xrpc/com.atproto.server.getSession")
            .match_header(
                "authorization",
                Matcher::AnyOf(vec![
                    Matcher::Exact("Bearer test-logged-access-jwt".to_string()),
                    Matcher::Exact("Bearer test-saved-access-jwt".to_string()),
                ]),
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                    "did": "did:plc:test_did",
                    "handle": "test.handle"
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
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                    "accessJwt": "test-refreshed-access-jwt",
                    "did": "did:plc:test_did",
                    "handle": "test.handle",
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
            .match_header(
                "authorization",
                Matcher::AnyOf(vec![
                    Matcher::Exact("Bearer test-logged-access-jwt".to_string()),
                    Matcher::Exact("Bearer test-saved-access-jwt".to_string()),
                    Matcher::Exact("Bearer test-refreshed-access-jwt".to_string()),
                    Matcher::Missing,
                ]),
            )
            .match_query(Matcher::AllOf(vec![
                Matcher::UrlEncoded("depth".to_string(), "1".to_string()),
                Matcher::UrlEncoded("parentHeight".to_string(), "20".to_string()),
                Matcher::UrlEncoded("uri".to_string(), TEST_THREAD_URI.to_string()),
            ]))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body_from_file("test_data/thread_3leb44umzuc2l.json5")
            .create_async()
            .await;
        server
    }

    #[tokio::test]
    async fn test_get_post_thread() {
        let mut server = Server::new_async().await;
        mock_get_session(&mut server).await;
        mock_refresh_session(&mut server).await;
        mock_get_post_thread(&mut server).await;
        let agent = create_test_agent(&server).await;
        let res = get_post_thread(agent, TEST_THREAD_URI.to_string()).await;
        assert!(res.is_ok());
        assert_eq!(
            res.unwrap().post.uri,
            "at://did:plc:xn5b64qpivpq55wumwf6wdjg/app.bsky.feed.post/3leb44umzuc2l"
        );
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
