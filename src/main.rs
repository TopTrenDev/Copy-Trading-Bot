use raypump_copytrading_bot::{
    common::{
        config::{tg_bot, Config},
        constants::RUN_MSG,
    },
    engine::monitor::copytrader_pumpfun,
    tg_bot,
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
    tg_bot::run(bot).await;

    // copytrader_pumpfun(
    //     &config.rpc_wss,
    //     config.app_state,
    //     config.token_percent,
    //     config.slippage,
    //     config.targetlist,
    // )
    // .await;
}
