use crate::{
    common::config::{Config, SwapConfig, SUBSCRIPTION_MSG},
    dex::pump_fun::Pump,
    telegram::send_msg,
    utils::file::{read_info, write_info},
};
use anyhow::Result;
use chrono::Utc;
use colored::Colorize;
use futures_util::{stream::StreamExt, SinkExt};
use serde_json::{json, to_string, Value};
use spl_token::amount_to_ui_amount;
use teloxide::{types::ChatId, Bot};
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
        let slot = json["params"]["result"]["slot"].as_u64().unwrap_or(0);
        let signature = json["params"]["result"]["signature"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let mut target = String::new();
        let mut mint = String::new();
        let mut bonding_curve = String::new();

        if let Some(account_keys) = json["params"]["result"]["transaction"]["transaction"]
            ["message"]["accountKeys"]
            .as_array()
        {
            if let Some(account_key) = account_keys
                .iter()
                .find(|key| key["signer"].as_bool().unwrap_or(false))
            {
                target = account_key["pubkey"].as_str().unwrap_or("").to_string();
            }
        }

        if let Some(post_token_balances) =
            json["params"]["result"]["transaction"]["meta"]["postTokenBalances"].as_array()
        {
            for balance in post_token_balances {
                let owner = balance["owner"].as_str().unwrap_or("");
                if owner != target {
                    bonding_curve = owner.to_string();
                }
                if owner == target || owner == bonding_curve {
                    mint = balance["mint"].as_str().unwrap_or("").to_string();
                }
            }
        }

        let token_post_amount = json["params"]["result"]["transaction"]["meta"]
            ["postTokenBalances"]
            .as_array()
            .and_then(|balances| {
                balances
                    .iter()
                    .find(|b| b["owner"] == target)
                    .and_then(|b| b["uiTokenAmount"]["uiAmount"].as_f64())
            })
            .unwrap_or(0.0);

        let token_pre_amount = json["params"]["result"]["transaction"]["meta"]["preTokenBalances"]
            .as_array()
            .and_then(|balances| {
                balances
                    .iter()
                    .find(|b| b["owner"] == target)
                    .and_then(|b| b["uiTokenAmount"]["uiAmount"].as_f64())
            })
            .unwrap_or(0.0);

        let bonding_curve_index = json["params"]["result"]["transaction"]["transaction"]["message"]
            ["accountKeys"]
            .as_array()
            .and_then(|keys| {
                keys.iter()
                    .position(|key| key["pubkey"].as_str().unwrap_or("") == bonding_curve)
            })
            .unwrap_or(0);

        let sol_post_amount = json["params"]["result"]["transaction"]["meta"]["postBalances"]
            .as_array()
            .and_then(|balances| balances.get(bonding_curve_index))
            .and_then(|b| b.as_u64())
            .unwrap_or(0);

        let sol_pre_amount = json["params"]["result"]["transaction"]["meta"]["preBalances"]
            .as_array()
            .and_then(|balances| balances.get(bonding_curve_index))
            .and_then(|b| b.as_u64())
            .unwrap_or(0);

        Ok(Self {
            slot,
            signature,
            target,
            mint,
            token_amount_list: TokenAmountList {
                token_pre_amount,
                token_post_amount,
            },
            sol_amount_list: SolAmountList {
                sol_pre_amount,
                sol_post_amount,
            },
        })
    }
}

pub async fn copytrader_pumpfun(bot: Bot, chat_id: ChatId) -> Result<()> {
    let config_guard = Config::get().await;
    let Config {
        rpc_wss,
        app_state,
        token_percent,
        slippage,
    } = &*config_guard;
    println!("================================");

    // WebSocket setup
    let (ws_stream, _) = match connect_async(&*rpc_wss).await {
        Ok(result) => result,
        Err(e) => {
            let error_msg = format!("Failed to connect to WebSocket '{}': {}", rpc_wss, e)
                .red()
                .to_string();
            println!("{}", error_msg);
            return Err(anyhow::anyhow!("WebSocket connection failed: {}", e));
        }
    };
    let (mut write, mut read) = ws_stream.split();
    if let Err(e) = write
        .send(WsMessage::Text(SUBSCRIPTION_MSG.to_string().into()))
        .await
    {
        println!("Failed to send subscription message: {}", e);
    }

    let prefix = "[PUMPFUN-MONITOR] => ".blue().bold().to_string();
    if let Err(e) = send_msg(
        bot.clone(),
        chat_id,
        prefix.clone(),
        "[STARTED. MONITORING]...".blue().bold().to_string(),
    )
    .await
    {
        println!("Error: {}", e);
    }

    // Subscription loop
    while let Some(msg) = read.next().await {
        // Read info from data.json
        let mut info: Value = match read_info(None).await {
            Ok(info) => info,
            Err(e) => {
                println!("Failed to read info: {}", e);
                break;
            }
        };

        let mut targetlist: Vec<String> = Vec::new();

        if let Some(user_data) = info.get_mut(chat_id.to_string()) {
            let usage = user_data.get("usage").and_then(|v| v.as_u64()).unwrap_or(0);
            if usage > 2 {
                if let Err(e) = send_msg(
                    bot.clone(),
                    chat_id,
                    "[Expired] => ".red().bold().to_string(),
                    "[Your credit limit has been exhausted]..."
                        .red()
                        .bold()
                        .to_string(),
                )
                .await
                {
                    println!("Error: {}", e);
                }
                break;
            }

            if let Some(target_address) = user_data.get("target_address").and_then(|v| v.as_str()) {
                targetlist.push(target_address.to_string()); // Convert &str to String
            } else {
                println!("No valid target_address found for chat_id {}", chat_id);
            }
        }

        let msg = msg?;
        let swapx = Pump::new(
            app_state.rpc_nonblocking_client.clone(),
            app_state.rpc_client.clone(),
            app_state.wallet.clone(),
        );

        if let WsMessage::Text(text) = msg {
            let start_time = Instant::now();
            let json: Value = match serde_json::from_str(&text) {
                Ok(json) => json,
                Err(e) => {
                    if let Err(e) = send_msg(
                        bot.clone(),
                        chat_id,
                        prefix.clone(),
                        format!("Error parsing WebSocket message: {}", e)
                            .red()
                            .italic()
                            .to_string(),
                    )
                    .await
                    {
                        println!("Error: {}", e);
                    }
                    continue;
                }
            };

            if json["params"]["result"]["transaction"]["transaction"]["message"]["accountKeys"]
                .as_array()
                .is_some()
            {
                let trade_info = match TradeInfoFromToken::from_json(json.clone()) {
                    Ok(info) => info,
                    Err(e) => {
                        if let Err(e) = send_msg(
                            bot.clone(),
                            chat_id,
                            prefix.clone(),
                            format!("Error parsing transaction: {}", e)
                                .red()
                                .italic()
                                .to_string(),
                        )
                        .await
                        {
                            println!("Error: {}", e);
                        }
                        continue;
                    }
                };

                if targetlist.contains(&trade_info.target) {
                    if let Err(e) = send_msg(
                        bot.clone(),
                        chat_id,
                        prefix.clone(),
                        format!("[PARSING]({}): {:?}", trade_info.mint, start_time.elapsed()),
                    )
                    .await
                    {
                        println!("Error: {}", e);
                    }

                    let sig = trade_info.signature.replace("\"", "");
                    if let Err(e) = send_msg(
                        bot.clone(),
                        chat_id,
                        prefix.clone(),
                        format!(
                            "[TARGET]({}): https://solscan.io/tx/{} :: {}",
                            trade_info.mint,
                            sig,
                            Utc::now()
                        ),
                    )
                    .await
                    {
                        println!("Error: {}", e);
                    }

                    let token_pre_amount = trade_info.token_amount_list.token_pre_amount;
                    let token_post_amount = trade_info.token_amount_list.token_post_amount;
                    let swap_config = if token_pre_amount < token_post_amount {
                        let sol_amount_lamports = trade_info.sol_amount_list.sol_post_amount
                            - trade_info.sol_amount_list.sol_pre_amount;
                        let sol_amount = amount_to_ui_amount(sol_amount_lamports, 9);
                        let amount_in = sol_amount * token_percent / 100.0;
                        SwapConfig {
                            swap_direction: SwapDirection::Buy,
                            in_type: SwapInType::Qty,
                            amount_in,
                            slippage: *slippage,
                            use_jito: true,
                        }
                    } else {
                        let token_amount = token_pre_amount - token_post_amount;
                        let amount_in = token_amount * token_percent / 100.0;
                        SwapConfig {
                            swap_direction: SwapDirection::Sell,
                            in_type: SwapInType::Qty,
                            amount_in,
                            slippage: *slippage,
                            use_jito: true,
                        }
                    };

                    if let Err(e) = send_msg(
                        bot.clone(),
                        chat_id,
                        prefix.clone(),
                        format!(
                            "[EXTRACTING]({}): {:?}",
                            trade_info.mint,
                            start_time.elapsed()
                        ),
                    )
                    .await
                    {
                        println!("Error: {}", e);
                    }

                    let bot_clone = bot.clone();
                    let prefix_clone = prefix.clone();
                    let swapx_clone = swapx.clone();
                    let swap_config_clone = swap_config.clone();
                    let mint_str = trade_info.mint.clone();
                    let chat_id_str = chat_id.to_string();
                    tokio::spawn(async move {
                        match swapx_clone
                            .swap_by_mint(&mint_str, swap_config_clone, start_time)
                            .await
                        {
                            Ok(res) => {
                                // Update usage for this chat ID
                                if let Some(user_data) = info.get_mut(&chat_id_str) {
                                    let usage = user_data
                                        .get("usage")
                                        .and_then(|v| v.as_u64())
                                        .unwrap_or(0)
                                        + 1;
                                    if let Some(obj) = user_data.as_object_mut() {
                                        obj.insert("usage".to_string(), json!(usage));
                                    }
                                } else {
                                    // If chat_id doesn't exist, create a new entry
                                    info[&chat_id_str] = json!({ "usage": 1 });
                                }

                                // Write updated info back
                                if let Err(e) =
                                    write_info(to_string(&info).unwrap_or_default(), None).await
                                {
                                    println!("Failed to write info: {}", e);
                                    return;
                                }

                                // Get usage for display (after update)
                                let usage = info
                                    .get(&chat_id_str)
                                    .and_then(|v| v.get("usage"))
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(1);

                                let message = format!(
                                    "\n\t * [SUCCESSFUL-COPIED] => TX_HASH: (https://solscan.io/tx/{}) \n\t * [POOL] => ({}) \n\t * [COPIED] => {} :: ({:?}). \n\t [USAGE] => {}",
                                    &res[0], mint_str, Utc::now(), start_time.elapsed(), usage
                                ).green().to_string();

                                if let Err(e) =
                                    send_msg(bot_clone, chat_id, prefix_clone, message).await
                                {
                                    println!("Error sending success message: {}", e);
                                }
                            }
                            Err(e) => {
                                let message = format!("Skip {}: {}", mint_str, e)
                                    .red()
                                    .italic()
                                    .to_string();
                                if let Err(e) =
                                    send_msg(bot_clone, chat_id, prefix_clone, message).await
                                {
                                    println!("Error sending error message: {}", e);
                                }
                            }
                        }
                    });
                }
            }
        }
    }

    Ok(())
}
