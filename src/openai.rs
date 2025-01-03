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
    use super::*;
    use crate::post::Post;

    #[test]
    fn test_generate_prompt() {
        let mut thread = VecDeque::new();
        thread.push_back(Post {
            handle: "user-1".to_string(),
            idx: 1,
            text: "Hello".to_string(),
            uri: "at://uri1".to_string(),
        });
        thread.push_back(Post {
            handle: "user-2".to_string(),
            idx: 2,
            text: "World".to_string(),
            uri: "at://uri2".to_string(),
        });
        let prompt = generate_prompt(&thread);
        assert_eq!(prompt, "```\n1：Hello\n2：World\n```\n");
    }
}
