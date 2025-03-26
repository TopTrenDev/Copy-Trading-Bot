use anyhow::Result;
use dotenv::dotenv;
use lazy_static::lazy_static;
use reqwest::Error;
use serde::Deserialize;
use serde_json::{json, Value};
use solana_sdk::{commitment_config::CommitmentConfig, signature::Keypair, signer::Signer};
use std::{env, sync::Arc};
use teloxide::prelude::*;
use tokio::sync::{Mutex, OnceCell};

use crate::{
    common::{constants::INIT_MSG, logger::Logger},
    dex::pump_fun::PUMP_PROGRAM,
    engine::swap::{SwapDirection, SwapInType},
};

static GLOBAL_CONFIG: OnceCell<Mutex<Config>> = OnceCell::const_new();

pub struct Config {
    pub rpc_wss: String,
    pub app_state: AppState,
    pub token_percent: f64,
    pub slippage: u64,
}

impl Config {
    pub async fn new() -> &'static Mutex<Config> {
        GLOBAL_CONFIG
            .get_or_init(|| async {
                let init_msg = INIT_MSG;
                println!("{}", init_msg);

                dotenv().ok(); // Load .env file

                let logger = Logger::new("[INIT] => ".to_string()); // Simplified color handling

                let rpc_wss = import_env_var("RPC_WSS");
                let slippage = import_env_var("SLIPPAGE").parse::<u64>().unwrap_or(5);
                let solana_price = create_coingecko_proxy().await.unwrap_or(200_f64);
                let rpc_client = create_rpc_client().unwrap();
                let rpc_nonblocking_client = create_nonblocking_rpc_client().await.unwrap();
                let wallet = import_wallet().unwrap();
                let balance = rpc_nonblocking_client
                    .get_account(&wallet.pubkey())
                    .await
                    .unwrap()
                    .lamports; // Adjusted to match dummy struct

                let wallet_cloned = wallet.clone();
                let token_percent = import_env_var("TOKEN_PERCENTAGE")
                    .parse::<f64>()
                    .unwrap_or(1_f64);

                let app_state = AppState {
                    rpc_client,
                    rpc_nonblocking_client,
                    wallet,
                };

                logger.log(format!(
                    "[COPYTRADER ENVIRONMENT]: \n\t\t\t\t [Web Socket RPC]: {},
                \n\t\t\t\t * [Wallet]: {:?}, * [Balance]: {} Sol, 
                \n\t\t\t\t * [Slippage]: {}, * [Solana]: {},
                \n\t\t\t\t * [Amount(%)]: {}",
                    rpc_wss,
                    wallet_cloned.pubkey(),
                    balance as f64 / 1_000_000_000_f64,
                    slippage,
                    solana_price,
                    token_percent,
                ));

                Mutex::new(Config {
                    rpc_wss,
                    app_state,
                    token_percent,
                    slippage,
                })
            })
            .await
    }

    pub async fn get() -> tokio::sync::MutexGuard<'static, Config> {
        GLOBAL_CONFIG
            .get()
            .expect("Config not initialized")
            .lock()
            .await
    }
}

pub const JUP_PUBKEY: &str = "JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4";

lazy_static! {
    pub static ref SUBSCRIPTION_MSG: Value = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "transactionSubscribe",
        "params": [
            {
                "failed": false,
                "accountInclude": [PUMP_PROGRAM],
                "accountExclude": [JUP_PUBKEY],
            },
            {
                "commitment": "processed",
                "encoding": "jsonParsed",
                "transactionDetails": "full",
                "maxSupportedTransactionVersion": 0
            }
        ]
    });
}

#[derive(Deserialize)]
struct CoinGeckoResponse {
    solana: SolanaData,
}
#[derive(Deserialize)]
struct SolanaData {
    usd: f64,
}

#[derive(Clone)]
pub struct AppState {
    pub rpc_client: Arc<solana_client::rpc_client::RpcClient>,
    pub rpc_nonblocking_client: Arc<solana_client::nonblocking::rpc_client::RpcClient>,
    pub wallet: Arc<Keypair>,
}

#[derive(Clone)]
pub struct SwapConfig {
    pub swap_direction: SwapDirection,
    pub in_type: SwapInType,
    pub amount_in: f64,
    pub slippage: u64,
    pub use_jito: bool,
}

pub fn import_env_var(key: &str) -> String {
    env::var(key).unwrap_or_else(|e| panic!("Environment variable {} is not set: {}", key, e))
}

pub fn create_rpc_client() -> Result<Arc<solana_client::rpc_client::RpcClient>> {
    let rpc_https = import_env_var("RPC_HTTPS");
    let rpc_client = solana_client::rpc_client::RpcClient::new_with_commitment(
        rpc_https,
        CommitmentConfig::processed(),
    );
    Ok(Arc::new(rpc_client))
}

pub async fn create_coingecko_proxy() -> Result<f64, Error> {
    let url = "https://api.coingecko.com/api/v3/simple/price?ids=solana&vs_currencies=usd";

    let response = reqwest::get(url).await?;

    let body = response.json::<CoinGeckoResponse>().await?;
    // Get SOL price in USD
    let sol_price = body.solana.usd;
    Ok(sol_price)
}

pub async fn create_nonblocking_rpc_client(
) -> Result<Arc<solana_client::nonblocking::rpc_client::RpcClient>> {
    let rpc_https = import_env_var("RPC_HTTPS");
    let rpc_client = solana_client::nonblocking::rpc_client::RpcClient::new_with_commitment(
        rpc_https,
        CommitmentConfig::processed(),
    );
    Ok(Arc::new(rpc_client))
}

pub fn import_wallet() -> Result<Arc<Keypair>> {
    let priv_key = import_env_var("PRIVATE_KEY");
    let wallet: Keypair = Keypair::from_base58_string(priv_key.as_str());

    Ok(Arc::new(wallet))
}

pub fn tg_bot() -> Result<Bot> {
    let bot_token = import_env_var("TG_TOKEN");
    let bot = Bot::new(bot_token);
    Ok(bot)
}
