mod api;
mod openai;
mod post;
mod util;

use crate::openai::openai_locate_sidetracker;
use dotenv::dotenv;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    pretty_env_logger::init();
    let agent = api::must_create_agent().await?;
    let res = api::get_post_thread(
        agent,
        "at://nghua.me/app.bsky.feed.post/3leb44umzuc2l".to_string(),
        // "at://demishuyan.bsky.social/app.bsky.feed.post/3lem7oosaz22t".to_string(),
    )
        .await?;

    let thread = post::flatten_thread(&res);

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
