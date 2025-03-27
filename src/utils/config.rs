use anyhow::Result;
use dotenv::dotenv;
use lazy_static::lazy_static;
use reqwest::Error;
use serde::Deserialize;
use serde_json::{json, Value};
use solana_sdk::{commitment_config::CommitmentConfig, signature::Keypair};
use std::sync::Arc;
use tokio::sync::{Mutex, OnceCell};

use crate::{
    dex::pump_fun::PUMP_PROGRAM,
    engine::swap::{SwapDirection, SwapInType},
    utils::{constants::INIT_MSG, env::read_env, logger::Logger},
};

static GLOBAL_CONFIG: OnceCell<Mutex<Config>> = OnceCell::const_new();

pub struct Config {
    pub rpc_wss: String,
    pub rpc_client: Arc<solana_client::rpc_client::RpcClient>,
    pub rpc_nonblocking_client: Arc<solana_client::nonblocking::rpc_client::RpcClient>,
    pub token_percent: f64,
    pub slippage: u64,
    pub jito_url: String,
    pub jito_tip_amount: f64,
}

impl Config {
    pub async fn new() -> &'static Mutex<Config> {
        GLOBAL_CONFIG
            .get_or_init(|| async {
                let init_msg = INIT_MSG;
                println!("{}", init_msg);

                dotenv().ok(); // Load .env file

                let logger = Logger::new("[INIT] => ".to_string()); // Simplified color handling

                let (
                    rpc_https,
                    rpc_wss,
                    commitment,
                    slippage,
                    token_percent,
                    jito_url,
                    jito_tip_amount,
                ) = read_env();
                let solana_price = create_coingecko_proxy().await.unwrap_or(200_f64);
                let rpc_client = create_rpc_client(rpc_https.clone(), commitment).unwrap();
                let rpc_nonblocking_client =
                    create_nonblocking_rpc_client(rpc_https.clone(), commitment).unwrap();

                logger.log(format!(
                    "[COPYTRADER ENVIRONMENT]: \n\t\t\t\t [Web Socket RPC]: {},
                \n\t\t\t\t * [Slippage]: {}, * [Solana]: {},
                \n\t\t\t\t * [Amount(%)]: {}",
                    rpc_wss, slippage, solana_price, token_percent,
                ));

                Mutex::new(Config {
                    rpc_wss,
                    rpc_client,
                    rpc_nonblocking_client,
                    token_percent,
                    slippage,
                    jito_url,
                    jito_tip_amount,
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

pub fn create_rpc_client(
    rpc: String,
    commitment: CommitmentConfig,
) -> Result<Arc<solana_client::rpc_client::RpcClient>> {
    let rpc_client = solana_client::rpc_client::RpcClient::new_with_commitment(rpc, commitment);
    Ok(Arc::new(rpc_client))
}

pub fn create_nonblocking_rpc_client(
    rpc: String,
    commitment: CommitmentConfig,
) -> Result<Arc<solana_client::nonblocking::rpc_client::RpcClient>> {
    let rpc_client =
        solana_client::nonblocking::rpc_client::RpcClient::new_with_commitment(rpc, commitment);
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
