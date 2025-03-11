use crate::{
    common::{
        config::{AppState, SwapConfig, SUBSCRIPTION_MSG},
        logger::Logger,
        targetlist::Targetlist,
    },
    dex::pump_fun::Pump,
};
use anyhow::Result;
use chrono::Utc;
use colored::Colorize;
use futures_util::{stream::StreamExt, SinkExt};
use serde_json::Value;
use spl_token::amount_to_ui_amount;
use tokio::time::Instant;
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};

use super::swap::{SwapDirection, SwapInType};

#[derive(Clone, Debug)]
pub struct TradeInfoFromToken {
    pub slot: u64,
    pub signature: String,
    pub target: String,
    pub mint: String,
    pub token_amount_list: TokenAmountList,
    pub sol_amount_list: SolAmountList,
}

#[derive(Clone, Debug)]
pub struct TokenAmountList {
    token_pre_amount: f64,
    token_post_amount: f64,
}

#[derive(Clone, Debug)]
pub struct SolAmountList {
    sol_pre_amount: u64,
    sol_post_amount: u64,
}

impl TradeInfoFromToken {
    pub fn from_json(json: Value) -> Result<Self> {
        let slot = json["params"]["result"]["slot"].as_u64().unwrap();
        let signature = json["params"]["result"]["signature"].clone().to_string();
        let mut target = String::new();
        let mut mint = String::new();
        let mut bonding_curve = String::new();

        // Retrieve Target Wallet Pubkey
        let account_keys = json["params"]["result"]["transaction"]["transaction"]["message"]
            ["accountKeys"]
            .as_array()
            .expect("Failed to get account keys");
        if let Some(account_key) = account_keys
            .iter()
            .find(|account_key| account_key["signer"].as_bool().unwrap())
        {
            target = account_key["pubkey"].as_str().unwrap().to_string();
        }

        if let Some(post_token_balances) =
            json["params"]["result"]["transaction"]["meta"]["postTokenBalances"].as_array()
        {
            for post_token_balance in post_token_balances.iter() {
                let owner = post_token_balance["owner"].as_str().unwrap();

                if owner != target {
                    bonding_curve = owner.to_string();
                }

                if owner == target || owner == bonding_curve {
                    mint = post_token_balance["mint"]
                        .as_str()
                        .unwrap_or("")
                        .to_string();
                }
            }
        }

        let token_post_amount = json["params"]["result"]["transaction"]["meta"]
            ["postTokenBalances"]
            .as_array()
            .and_then(|post_token_balances| {
                post_token_balances
                    .iter()
                    .find(|post_token_balance| post_token_balance["owner"] == target)
                    .and_then(|post_token_balance| {
                        post_token_balance["uiTokenAmount"]["uiAmount"].as_f64()
                    })
            })
            .unwrap_or(0_f64);

        let token_pre_amount = json["params"]["result"]["transaction"]["meta"]["preTokenBalances"]
            .as_array()
            .and_then(|pre_token_balances| {
                pre_token_balances
                    .iter()
                    .find(|pre_token_balance| pre_token_balance["owner"] == target)
                    .and_then(|pre_token_balance| {
                        pre_token_balance["uiTokenAmount"]["uiAmount"].as_f64()
                    })
            })
            .unwrap_or(0_f64);

        let bonding_curve_index = account_keys
            .iter()
            .position(|account_key| account_key["pubkey"].as_str().unwrap() == bonding_curve)
            .unwrap_or(0);

        let sol_post_amount = json["params"]["result"]["transaction"]["meta"]["postBalances"]
            .as_array()
            .and_then(|post_balances| post_balances.get(bonding_curve_index))
            .and_then(|post_balance| post_balance.as_u64())
            .unwrap_or(0_u64);

        let sol_pre_amount = json["params"]["result"]["transaction"]["meta"]["preBalances"]
            .as_array()
            .and_then(|pre_balances| pre_balances.get(bonding_curve_index))
            .and_then(|pre_balance| pre_balance.as_u64())
            .unwrap_or(0_u64);

        let token_amount_list = TokenAmountList {
            token_pre_amount,
            token_post_amount,
        };

        let sol_amount_list = SolAmountList {
            sol_pre_amount,
            sol_post_amount,
        };

        Ok(Self {
            slot,
            signature,
            target,
            mint,
            token_amount_list,
            sol_amount_list,
        })
    }
}

pub async fn copytrader_pumpfun(
    rpc_wss: &str,
    app_state: AppState,
    token_percent: f64,
    slippage: u64,
    targetlist: Targetlist,
) {
    // INITIAL SETTING FOR SUBSCIBE
    // -----------------------------------------------------------------------------------------------------------------------------
    let (ws_stream, _) = connect_async(rpc_wss)
        .await
        .expect("Failed to connect to WebSocket server");
    let (mut write, mut read) = ws_stream.split();
    write
        .send(SUBSCRIPTION_MSG.to_string().into())
        .await
        .expect("Failed to send subscription message");

    let rpc_nonblocking_client = app_state.clone().rpc_nonblocking_client;
    let rpc_client = app_state.clone().rpc_client;
    let wallet = app_state.clone().wallet;
    let swapx = Pump::new(
        rpc_nonblocking_client.clone(),
        rpc_client.clone(),
        wallet.clone(),
    );

    let logger = Logger::new("[PUMPFUN-MONITOR] => ".blue().bold().to_string());
    logger.log("[STARTED. MONITORING]...".blue().bold().to_string());

    // NOW SUBSCRIBE
    // -----------------------------------------------------------------------------------------------------------------------------
    while let Some(Ok(msg)) = read.next().await {
        if let WsMessage::Text(text) = msg {
            let start_time = Instant::now();
            let json: Value = serde_json::from_str(&text).unwrap();

            if let Some(_account_keys) = json["params"]["result"]["transaction"]["transaction"]
                ["message"]["accountKeys"]
                .as_array()
            {
                let trade_info = match TradeInfoFromToken::from_json(json.clone()) {
                    Ok(info) => info,
                    Err(e) => {
                        logger.log(
                            format!("Error in parsing txn: {}", e)
                                .red()
                                .italic()
                                .to_string(),
                        );
                        continue;
                    }
                };
                // CHECK IF THIS IS THE TXN OF TARGET WALLET.
                // -------------
                if targetlist.is_listed_on_target(&trade_info.target) {
                    logger.log(format!(
                        "[PARSING]({}): {:?}",
                        trade_info.mint,
                        start_time.elapsed()
                    ));
                    let sig = trade_info.signature.replace("\"", "");

                    logger.log(format!(
                        "[TARGET]({}): https://solscan.io/tx/{} :: {}",
                        trade_info.mint,
                        &sig,
                        Utc::now()
                    ));
                    // CHECK IF THIS IS BUY OR SELL TXN
                    // ------------
                    let token_pre_amount = trade_info.token_amount_list.token_pre_amount;
                    let token_post_amount = trade_info.token_amount_list.token_post_amount;
                    let swap_config = if token_pre_amount < token_post_amount {
                        // BUY TXN
                        // ---------------
                        let sol_amount_lamports = trade_info.sol_amount_list.sol_post_amount
                            - trade_info.sol_amount_list.sol_pre_amount;
                        let sol_amount = amount_to_ui_amount(sol_amount_lamports, 9);
                        let amount_in = sol_amount * token_percent / 100_f64;
                        SwapConfig {
                            swap_direction: SwapDirection::Buy,
                            in_type: SwapInType::Qty,
                            amount_in,
                            slippage,
                            use_jito: true,
                        }
                    } else {
                        // SELL TXN
                        // ---------------
                        let token_amount = token_pre_amount - token_post_amount;
                        let amount_in = token_amount * token_percent / 100_f64;
                        SwapConfig {
                            swap_direction: SwapDirection::Sell,
                            in_type: SwapInType::Qty,
                            amount_in,
                            slippage,
                            use_jito: true,
                        }
                    };

                    logger.log(format!(
                        "[EXTRACTING]({}): {:?}",
                        trade_info.mint,
                        start_time.elapsed()
                    ));

                    let swapx_clone = swapx.clone();
                    let logger_clone = logger.clone();
                    let swap_config_clone = swap_config.clone();
                    let mint_str = trade_info.mint.clone();
                    let task = tokio::spawn(async move {
                        match swapx_clone
                            .swap_by_mint(&mint_str, swap_config_clone, start_time)
                            .await
                        {
                            Ok(res) => {
                                logger_clone.log(format!(
                                        "\n\t * [SUCCESSFUL-COPIED] => TX_HASH: (https://solscan.io/tx/{}) \n\t * [POOL] => ({}) \n\t * [COPIED] => {} :: ({:?}).",
                                        &res[0], mint_str, Utc::now(), start_time.elapsed()
                                    ).green().to_string());
                            }
                            Err(e) => {
                                logger_clone.log(
                                    format!("Skip {}: {}", mint_str.clone(), e)
                                        .red()
                                        .italic()
                                        .to_string(),
                                );
                            }
                        }
                    });
                    drop(task);
                }
            }
        }
    }
}
