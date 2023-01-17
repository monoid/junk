mod store;

use std::path::PathBuf;
use clap::Parser;

use teloxide::{prelude::*, requests::ResponseResult, types::Me};

use crate::store::{JsonFileStore, Store};

#[derive(Parser, Debug)]
struct Args {
    #[clap(parse(from_os_str))]
    output: PathBuf,
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    let args = Args::from_args();
    run(args).await
}

async fn run(args: Args) {
    let bot = Bot::from_env();
    let Me { user: bot_user, .. } = bot.get_me().await.unwrap();
    let bot_name = bot_user.username.expect("Bots must have usernames");

    log::info!("Starting the bot {}", bot_name);

    let store = JsonFileStore::new(&args.output).expect("Failed to open the output file");

    log::info!("Storing data to {:?}", args.output);

    teloxide::repl(bot, move |message: Message| {
        // Clone for the async generator below.
        let store = store.clone();

        async move {
            let chat = &message.chat;
            // Ignore non-group messages
            if chat.is_private() {
                log::info!("User ID: {}", chat.id);
                return ResponseResult::<()>::Ok(());
            }

            log::debug!("Message: {:?}", message);

            store
                .store(message)
                .await
                .expect("Failed to write the message");

            Ok(())
        }
    })
    .await;
}
