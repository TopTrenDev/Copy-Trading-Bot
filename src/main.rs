use raypump_copytrading_bot::{
    common::{
        config::{tg_bot, Config},
        constants::RUN_MSG,
    },
    telegram,
};

#[tokio::main]
async fn main() {
    /* Initial Settings */
    Config::new().await;

    /* Running Bot */
    let run_msg = RUN_MSG;
    println!("{}", run_msg);

    let bot = tg_bot().unwrap();

    // Start the bot
    println!("Bot is running...");
    telegram::run(bot).await;
}
