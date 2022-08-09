mod store;

use std::path::PathBuf;
use structopt::StructOpt;

use teloxide::{prelude::*, requests::ResponseResult, types::Me};

use crate::store::{JsonFileStore, Store};

#[derive(StructOpt, Debug)]
struct Args {
    #[structopt(parse(from_os_str))]
    output: PathBuf,
}

#[tokio::main]
async fn main() {
    let args = Args::from_args();
    run(args).await
}

async fn run(args: Args) {
    teloxide::enable_logging!();
    let bot = Bot::from_env().auto_send();
    let Me { user: bot_user, .. } = bot.get_me().await.unwrap();
    let bot_name = bot_user.username.expect("Bots must have usernames");

    log::info!("Starting the bot {}", bot_name);

    let store = JsonFileStore::new(&args.output).expect("Failed to open the output file");

    log::info!("Storing data to {:?}", args.output);

    teloxide::repl(bot, move |message| {
        // Clone for the async generator below.
        let store = store.clone();

        async move {
            let chat = &message.update.chat;
            // Ignore non-group messages
            if chat.is_private() {
                log::debug!("Private: {:?}", message.update);
                log::info!("User ID: {}", chat.id);
                return ResponseResult::<()>::Ok(());
            }

            log::debug!("Message: {:?}", message.update);

            store
                .store(message.update)
                .await
                .expect("Failed to write the message");

            ResponseResult::<()>::Ok(())
        }
    })
    .await;
}
