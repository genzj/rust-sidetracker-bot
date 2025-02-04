mod api;
mod data;
mod openai;
mod post;
mod util;
mod session;

use crate::data::SideTracker;
use crate::openai::openai_locate_sidetracker;
use crate::post::PostLocator;
use clap::{Parser, Subcommand};
use dotenv::dotenv;
use log::debug;
use std::collections::VecDeque;
use std::error::Error;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
#[command(next_line_help = true)]
struct Cli {
    #[arg(short, long, action = clap::ArgAction::Count, default_value_t = 3)]
    /// verbosity of logging. This option can be repeated.
    /// 1 - Error,
    /// 2 - Warn,
    /// 3 - Info (default),
    /// 4 - Debug,
    /// 5 and more - Trace
    verbose: u8,

    #[arg(short, long)]
    /// disable all logs.
    quiet: bool,

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

    match cli.command {
        Commands::Check { thread } => {
            check(&PostLocator::from_url(&thread)?.at_uri()).await?;
        }
    }
    Ok(())
}

async fn check(thread: &str) -> Result<(), Box<dyn Error>> {
    let agent = api::must_create_agent().await?;
    let res = api::get_post_thread(
        agent,
        thread.to_string(),
    )
        .await?;

    let thread = post::FlattenedThread::from(&res);
    let posts = VecDeque::from(&thread);
    let result = SideTracker::new(
        openai_locate_sidetracker(&posts).await,
        thread.root.borrow().clone(),
        thread.entrance.borrow().clone(),
    );

    println!("{:?}", result.build_reply());
    Ok(())
}

fn set_verbosity(cli: &Cli) {
    let log_level = match (cli.quiet, cli.verbose) {
        (true, _) => log::LevelFilter::Off,
        (false, 1) => log::LevelFilter::Error,
        (false, 2) => log::LevelFilter::Warn,
        (false, 0 | 3) => log::LevelFilter::Info,
        (false, 4) => log::LevelFilter::Debug,
        (false, _) => log::LevelFilter::Trace,
    };

    log::set_max_level(log_level);
}
