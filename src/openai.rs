use crate::post::Post;
use crate::util;
use log::{debug, error};
use openai::chat::{ChatCompletion, ChatCompletionMessage, ChatCompletionMessageRole};
use openai::Credentials;
use std::collections::VecDeque;
use std::env;

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

pub async fn openai_locate_sidetracker(thread: &VecDeque<Post>) -> Option<Post> {
    // Relies on OPENAI_KEY and optionally OPENAI_BASE_URL.
    let credentials = Credentials::from_env();
    let messages = vec![
        ChatCompletionMessage {
            role: ChatCompletionMessageRole::System,
            content: Some(include_str!("../data/prompt.txt").to_string()),
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
        let idx = util::find_and_parse_first_integer(
            returned_message.content.clone().unwrap().trim().to_string(),
        );
        if let Some(idx) = idx {
            for p in thread.iter() {
                if p.idx == idx {
                    return Some(p.clone());
                }
            }
        }
        None
    } else {
        error!("error: {:#?}", chat_completion);
        None
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use atrium_api::types::string::{Cid, Did};

    use super::*;
    use crate::post::Post;

    #[test]
    fn test_generate_prompt() {
        let mut thread = VecDeque::new();
        thread.push_back(Post {
            cid: Cid::from_str("bafyreihvgtbjqmyo2ocpfic3rgjtvepbopaaaaawcccccsxxxxxw3nnjly").unwrap(),
            did: Did::from_str("did:plc:fkjudld5cgxxxxxxxxxxxxxx").unwrap(),
            handle: "user-1".to_string(),
            idx: 1,
            text: "Hello".to_string(),
            uri: "at://uri1".to_string(),
        });
        thread.push_back(Post {
            cid: Cid::from_str("bafyreihvgtbjqmyo2ocpfic3rgjtvepbopbbbbbwaaaaasyyyyyw3nnjly").unwrap(),
            did: Did::from_str("did:plc:fkjudld5cgyyyyyyyyyyyyyy").unwrap(),
            handle: "user-2".to_string(),
            idx: 2,
            text: "World".to_string(),
            uri: "at://uri2".to_string(),
        });
        let prompt = generate_prompt(&thread);
        assert_eq!(prompt, "```\n1：Hello\n2：World\n```\n");
    }
}
