mod api;
mod data;
mod openai;
mod post;
mod session;
mod util;

use crate::data::SideTracker;
use crate::openai::openai_locate_sidetracker;
use crate::post::PostLocator;
use atrium_api::app::bsky::feed::post::RecordData;
use clap::{Parser, Subcommand};
use dotenv::dotenv;
use log::debug;
use std::collections::VecDeque;
use std::error::Error;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
#[command(next_line_help = true)]
struct Cli {
    #[arg(short, long, action = clap::ArgAction::Count, default_value_t = 0, global = true)]
    /// verbosity of logging. This option can be repeated.
    /// 0 - (Default) no overriding, use env RUST_LOG or pretty_env_logger's default level
    /// 1 - Error,
    /// 2 - Warn,
    /// 3 - Info,
    /// 4 - Debug,
    /// 5 and more - Trace
    verbose: u8,

    #[arg(short, long, global = true)]
    /// disable all logs.
    quiet: bool,

    #[arg(short = 'n', long, global = true, env = "DRY_RUN")]
    /// disable all post creation features.
    dry_run: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// check a thread and exit
    Check {
        /// thread uri
        thread: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv().ok();
    pretty_env_logger::init();

    let cli = Cli::parse();
    set_verbosity(&cli);
    debug!("cli: {:?}", cli);

    let reply = match cli.command {
        Commands::Check { thread } => check(&PostLocator::from_url(&thread)?.at_uri()).await?,
    };
    if let Some(reply) = reply {
        if cli.dry_run {
            debug!("dry run: not posting");
            println!("{}", serde_json::to_string_pretty(&reply).unwrap());
        } else {
            let agent = api::must_create_agent().await?;
            debug!("posting reply: {:?}", reply);
            let result = api::create_record(agent, reply).await?;
            debug!("reply result: {:?}", result);
        }
    }
    Ok(())
}

async fn check(thread: &str) -> Result<Option<RecordData>, Box<dyn Error>> {
    let agent = api::must_create_agent().await?;
    let res = api::get_post_thread(agent, thread.to_string()).await?;

    let thread = post::FlattenedThread::from(&res);
    let posts = VecDeque::from(&thread);
    let result = SideTracker::new(
        openai_locate_sidetracker(&posts).await,
        thread.root.borrow().clone(),
        thread.entrance.borrow().clone(),
    );

    debug!("side tracking result {:?}", result);
    Ok(Some(result.build_reply()))
}

fn set_verbosity(cli: &Cli) {
    let log_level = match (cli.quiet, cli.verbose) {
        (true, _) => Some(log::LevelFilter::Off),
        // keep default value, which can be set by env RUST_LOG
        (false, 0) => None,
        (false, 1) => Some(log::LevelFilter::Error),
        (false, 2) => Some(log::LevelFilter::Warn),
        (false, 3) => Some(log::LevelFilter::Info),
        (false, 4) => Some(log::LevelFilter::Debug),
        (false, _) => Some(log::LevelFilter::Trace),
    };

    log_level.map(log::set_max_level);
}
