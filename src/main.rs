use atproto_api::{Agent, AtpAgent, Session};
use dotenv::dotenv;
use futures::FutureExt;
use log::{debug, error, info};
use openai::chat::{ChatCompletion, ChatCompletionMessage, ChatCompletionMessageRole};
use openai::Credentials;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::panic::AssertUnwindSafe;
use std::env;

#[derive(Debug, Clone, Eq, PartialEq)]
struct Post {
    handle: String,
    text: String,
    uri: String,
    idx: u32,
}

impl Post {
    pub fn new(handle: impl Into<String>, text: impl Into<String>, uri: impl Into<String>, idx: u32) -> Self {
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
        println!("最有可能的歪楼犯：{}\n罪证：{}\n现场还原：{}", p.handle, p.text, p.get_share_uri());
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
                }).catch_unwind().await;
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

async fn get_post_thread(
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

fn parse_post_text(post: &Value) -> &str {
    post["record"]["text"].as_str().unwrap().trim()
}

fn parse_post_uri(post: &Value) -> &str {
    post["uri"].as_str().unwrap()
}

fn parse_post_author_handle(post: &Value) -> &str {
    post["author"]["handle"].as_str().unwrap()
}

fn parse_embedded(post: &Value) -> Option<Post> {
    let value = &post["post"]["embed"]["record"];
    if value.is_object() && value["value"]["text"].to_string().trim().len() > 0 {
        Some(Post::new(
            value["author"]["handle"].to_string(),
            value["value"]["text"].to_string().trim(),
            value["uri"].to_string(),
            0,
        ))
    } else {
        None
    }
}

fn generate_prompt(thread: &VecDeque<Post>) -> String {
    let mut prompt = String::new();
    prompt.push_str("```\n");
    for p in thread.iter() {
        prompt.push_str(&format!("{}：{}\n", p.idx, p.text.replace("\n", "\\n")));
    }
    prompt.push_str("```\n");
    prompt
}

const OPENAI_MODEL_DEFAULT: &str = "gpt-4o-mini";

async fn openai_locate_sidetracker(thread: &VecDeque<Post>) -> Option<Post> {
    // Relies on OPENAI_KEY and optionally OPENAI_BASE_URL.
    let credentials = Credentials::from_env();
    let messages = vec![
        ChatCompletionMessage {
            role: ChatCompletionMessageRole::System,
            content: Some("论坛中的歪楼是指在论坛中的回复中有人故意跑题，提出与楼主本意看似相关而又无关的问题。下面是一个讨论中的若干回复，请你指出最有可能导致歪楼的回复。只需要回答对应回复前的数字序号，不做解释，不输出其他文字。如果所有回复都没有跑题，输出数字0。".to_string()),
            ..Default::default()
        },
        ChatCompletionMessage {
            role: ChatCompletionMessageRole::User,
            content: Some(generate_prompt(thread)),
        ..Default::default()
        },
    ];
    let model = env::var("OPENAI_MODEL").unwrap_or(OPENAI_MODEL_DEFAULT.to_string());
    debug!("using model {}", model);
    let chat_completion = ChatCompletion::builder(&model, messages)
        .credentials(credentials)
        .create()
        .await;
    if let Ok(output) = chat_completion {
        let returned_message = output.choices.first().unwrap().message.clone();
        debug!(
            "OpenAI response: {:#?}: {}",
            returned_message.role,
            returned_message.content.clone().unwrap().trim()
        );
        let idx = find_and_parse_first_integer(returned_message.content.clone().unwrap().trim().to_string());
        if let Some(idx) = idx {
            for p in thread.iter() {
                if p.idx == idx {
                    return Some(p.clone());
                }
            }
        }
        return None;
    } else {
        error!("error: {:#?}", chat_completion);
        None
    }
}

fn find_and_parse_first_integer(input: String) -> Option<u32> {
    let mut num_str = String::new();
    let mut found_number = false;

    for c in input.chars() {
        if c.is_digit(10) {
            num_str.push(c);
            found_number = true;
        } else if found_number {
            break;
        }
    }

    if let Ok(num) = num_str.parse::<u32>() {
        Some(num)
    } else {
        None
    }
}