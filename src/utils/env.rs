use anyhow::Result;
use solana_sdk::commitment_config::CommitmentConfig;
use std::env;
use teloxide::Bot;

pub fn import_env_var(key: &str) -> String {
    env::var(key).unwrap_or_else(|e| panic!("Environment variable {} is not set: {}", key, e))
}

pub fn read_env() -> (String, String, CommitmentConfig, u64, f64, String, String, String, f64) {
    let rpc_https = import_env_var("RPC_HTTPS");
    let rpc_wss = import_env_var("RPC_WSS");
    let commitment = match import_env_var("COMMITMENT").as_str() {
        "finalized" => CommitmentConfig::finalized(),
        "confirmed" => CommitmentConfig::confirmed(),
        _ => CommitmentConfig::processed(), // Default to processed for any other value
    };
    let slippage = import_env_var("SLIPPAGE").parse::<u64>().unwrap_or(5);
    let token_percent = import_env_var("TOKEN_PERCENTAGE")
        .parse::<f64>()
        .unwrap_or(1.0);
    let yellowstone_grpc_http = import_env_var("YELLOWSTONE_GRPC_HTTP");
    let yellowstone_grpc_token = import_env_var("YELLOWSTONE_GRPC_TOKEN");
    let jito_url = import_env_var("JITO_BLOCK_ENGINE_URL");
    let jito_tip_amount = import_env_var("JITO_TIP_AMOUNT")
        .parse::<f64>()
        .unwrap_or(0.001);

    (
        rpc_https,
        rpc_wss,
        commitment,
        slippage,
        token_percent,
        yellowstone_grpc_http,
        yellowstone_grpc_token,
        jito_url,
        jito_tip_amount,
    )
}

pub fn tg_bot() -> Result<Bot> {
    let bot_token = import_env_var("TG_TOKEN");
    let bot = Bot::new(bot_token);
    Ok(bot)
}
