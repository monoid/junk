use std::io::Write;
use std::ops::DerefMut;
use std::path::PathBuf;
use structopt::StructOpt;

use teloxide::{prelude::*, requests::ResponseResult, types::Me};

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

    let output = std::sync::Arc::new(std::sync::Mutex::new(
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&args.output)
            .expect("Failed to open the output file"),
    ));

    log::info!("Storing data to {:?}", args.output);

    teloxide::repl(bot, move |message| {
        // Clone for the async generator below.
        let output = output.clone();

        async move {
            let chat = &message.update.chat;
            // Ignore non-group messages
            if chat.is_private() {
                log::debug!("Private: {:?}", message.update);
                return ResponseResult::<()>::Ok(());
            }

            log::debug!("Message: {:?}", message.update);

            // Just run writing operation in the pool...  The pool and
            // the disk are the (unlikely) bottlenecks, but it is
            // unevitable, unless architectured differently.
            tokio::task::spawn_blocking(move || -> Result<(), std::io::Error> {
                let mut output = output.lock().unwrap_or_else(|e| e.into_inner());

                serde_json::to_writer(output.deref_mut(), &message.update)?;
                writeln!(&mut output)?;

                output.flush()?;
                // Sync may fail if output file is /dev/stdout.  One
                // can selectively ignore code 25 ENOTTY.
                let _ = output.sync_all();

                Ok(())
            })
            .await
            .expect("Failed to run a blocking writing operation")
            .expect("Failed to write the message");

            ResponseResult::<()>::Ok(())
        }
    })
    .await;
}
