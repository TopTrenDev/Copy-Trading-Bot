use serde_json::{json, to_string, Value};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::{bs58, signature::Keypair, signer::Signer};
use std::collections::HashMap;
use std::error::Error;
use teloxide::{
    dispatching::{
        dialogue::{self, InMemStorage},
        UpdateHandler,
    },
    prelude::*,
    types::MaybeInaccessibleMessage,
    utils::command::BotCommands,
};

use crate::common::logger::Logger;
use crate::engine::monitor::copytrader_pumpfun;
use crate::msg::{setting_op_keyboard, start_op_keyboard, SettingOp, StartOp};
use crate::utils::file::{read_info, write_info};

type MyDialogue = Dialogue<ChatState, InMemStorage<ChatState>>;
type HandlerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone, Default)]
enum ChatState {
    #[default]
    Start,
    StartCb,
    SettingCb,
    AddWallet,
    TargetSet,
    StopTrading,
}

#[derive(Debug)]
pub struct State {
    //chain Id -> token address -> user address -> subscribed users
    pub subs: HashMap<u32, ChatId>,
}

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", description = "Commands:")]
enum Command {
    #[command(description = "Display all commands.")]
    Help,
    #[command(description = "Start the copy-trading bot")]
    Start,
    #[command(description = "Stop the copy-trading bot")]
    Stop,
}

pub async fn run(bot: Bot) {
    Dispatcher::builder(bot, schema())
        .dependencies(dptree::deps![InMemStorage::<ChatState>::new()])
        .error_handler(LoggingErrorHandler::with_custom_text(
            "An error has occurred in the dispatcher",
        ))
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}

fn schema() -> UpdateHandler<Box<dyn std::error::Error + Send + Sync + 'static>> {
    use dptree::case;

    // Command
    let command_handler = teloxide::filter_command::<Command, _>()
        .branch(case![Command::Start].endpoint(start))
        .branch(case![Command::Help].endpoint(help))
        .branch(case![Command::Stop].endpoint(cancel))
        .branch(dptree::endpoint(invalid_command));

    // Text
    #[rustfmt::skip]
    let text_handler = Message::filter_text()
        .branch(case![ChatState::AddWallet].endpoint(add_wallet))
        .branch(case![ChatState::TargetSet].endpoint(target_set));

    // Information
    let message_handler = Update::filter_message()
        .branch(command_handler)
        .branch(text_handler)
        .branch(dptree::endpoint(invalid_state));

    let callback_query_handler = Update::filter_callback_query()
        // .branch(case![ChatState::ReceiveChainId].endpoint(receive_chain_id))
        .branch(case![ChatState::StartCb].endpoint(start_cb))
        .branch(case![ChatState::SettingCb].endpoint(setting_cb))
        .branch(case![ChatState::StopTrading].endpoint(stop_trading))
        .branch(dptree::endpoint(invalid_callback_query));

    dialogue::enter::<Update, InMemStorage<ChatState>, ChatState, _>()
        .branch(message_handler)
        .branch(callback_query_handler)
}

async fn invalid_state(_bot: Bot, msg: Message) -> HandlerResult {
    log::warn!("invalid state - Unable to handle the message: {:?}", msg);
    Ok(())
}

async fn invalid_command(bot: Bot, msg: Message) -> HandlerResult {
    bot.send_message(
        msg.chat.id,
        format!("❌ Wrong command - usage: \n{}", Command::descriptions()),
    )
    .await?;
    Ok(())
}

async fn invalid_callback_query(bot: Bot, q: CallbackQuery) -> HandlerResult {
    bot.answer_callback_query(q.id).await?;
    if let Some(maybe_msg) = q.message {
        match maybe_msg {
            MaybeInaccessibleMessage::Regular(msg) => {
                bot.edit_message_text(
                    msg.chat.id,
                    msg.id,
                    "❎ Conversation expired. Use /start to start over",
                )
                .await?;
            }
            MaybeInaccessibleMessage::Inaccessible(_) => {
                // Handle the case where the message is inaccessible (e.g., deleted or bot lacks access)
                // You could log this or ignore it since you can't edit it
                println!("Received an inaccessible message, cannot edit.");
            }
        }
    }
    Ok(())
}

async fn help(bot: Bot, msg: Message) -> HandlerResult {
    bot.send_message(msg.chat.id, Command::descriptions().to_string())
        .await?;
    Ok(())
}

async fn cancel(bot: Bot, msg: Message, dialogue: MyDialogue) -> HandlerResult {
    bot.send_message(msg.chat.id, "Cancelling the subscription process.")
        .await?;
    dialogue.exit().await?;
    Ok(())
}

async fn start(bot: Bot, msg: Message, dialogue: MyDialogue) -> HandlerResult {
    bot.send_message(msg.chat.id, "Welcome to copytrading bot with Rust")
        .reply_markup(start_op_keyboard())
        .await?;
    dialogue.update(ChatState::StartCb).await?;
    Ok(())
}

async fn start_cb(bot: Bot, dialogue: MyDialogue, q: CallbackQuery) -> HandlerResult {
    let chat_id = dialogue.chat_id().to_string();

    bot.answer_callback_query(q.id).await?; // todo await
    if let Some(op) = &q.data {
        match serde_json::from_str(op)? {
            StartOp::Run => run_trading(bot, dialogue).await?,
            StartOp::Stop => {
                let text = "Stopped the bot\n";
                bot.send_message(chat_id, text).await?;
                dialogue.update(ChatState::StopTrading).await?;
            }
            StartOp::Setting => {
                bot.send_message(chat_id, "Configuration Settings for Your Bot")
                    .reply_markup(setting_op_keyboard())
                    .await?;
                dialogue.update(ChatState::SettingCb).await?;
            }
            StartOp::Help => {}
        }
    }
    Ok(())
}

async fn setting_cb(bot: Bot, dialogue: MyDialogue, q: CallbackQuery) -> HandlerResult {
    let chat_id = dialogue.chat_id().to_string();
    bot.answer_callback_query(q.id).await?; // todo await
    if let Some(op) = &q.data {
        match serde_json::from_str(op)? {
            SettingOp::Wallet => {
                let info = read_info(None).await?;

                // Check if chat ID already exists
                let chat_id = dialogue.chat_id().to_string();
                if info.get(&chat_id).is_some() {
                    bot.send_message(chat_id, "Your wallet is already imported")
                        .await?;
                    return Ok(());
                }
                let text = "Please input Your wallet\n";
                bot.send_message(chat_id, text).await?;
                dialogue.update(ChatState::AddWallet).await?;
            }
            SettingOp::Target => {
                let text = "Target Wallet: Whale, Trader\n";
                bot.send_message(chat_id, text).await?;
                dialogue.update(ChatState::TargetSet).await?;
            }
        }
    }
    Ok(())
}

async fn add_wallet(bot: Bot, dialogue: MyDialogue, msg: Message) -> HandlerResult {
    let mut info = read_info(None).await?;

    // Check if chat ID already exists
    let chat_id = dialogue.chat_id().to_string();

    // Extract private key from message
    let Some(private_key) = msg.text().map(ToOwned::to_owned) else {
        bot.send_message(msg.chat.id, "Empty message error. Sorry, please try again")
            .await?;
        return Ok(());
    };
    let keypair_bytes = match bs58::decode(private_key.clone()).into_vec() {
        Ok(bytes) => bytes,
        Err(e) => {
            println!("Failed to decode base58 private key: {}", e);
            bot.send_message(
                msg.chat.id,
                format!("Invalid private key. {} Please try again.", e),
            )
            .await?;
            return Ok(()); // Exit after notifying user
        }
    };

    // Create Keypair from bytes
    let wallet = match Keypair::from_bytes(&keypair_bytes) {
        Ok(kp) => kp,
        Err(e) => {
            println!("Invalid private key bytes: {}", e);
            bot.send_message(
                msg.chat.id,
                format!("Invalid private key. {} Please try again.", e),
            )
            .await?;
            return Ok(()); // Exit after notifying user
        }
    };

    // Update JSON with new wallet data
    info[&chat_id] = json!({
        "private_key": &private_key,
        "usage": 0
    });

    write_info(to_string(&info)?, None).await?;

    // Send success message with public key
    let response = format!(
        "👛Your wallet\n {}\n is correctly imported",
        wallet.pubkey()
    );
    bot.send_message(msg.chat.id, response).await?;

    Ok(())
}

async fn target_set(bot: Bot, dialogue: MyDialogue, msg: Message) -> HandlerResult {
    let mut info = read_info(None).await?;

    // Check if chat ID already exists
    let chat_id = dialogue.chat_id();
    let user_info = info.get(chat_id.to_string());
    if let Some(user_data) = user_info {
        // Check for target_address
        if user_data.get("target_address").is_some() {
            bot.send_message(chat_id, "Target address is already set!")
                .await?;
            return Ok(());
        } else {
            // Extract target address from message
            let Some(target_address) = msg.text().map(ToOwned::to_owned) else {
                bot.send_message(chat_id, "Empty message error. Sorry, please try again")
                    .await?;
                return Ok(());
            };

            // Validate Solana public key (base58 decode and check length)
            let pubkey_bytes = match bs58::decode(&target_address).into_vec() {
                Ok(bytes) if bytes.len() == 32 => bytes,
                Ok(bytes) => {
                    println!("Invalid Solana public key length: {} bytes", bytes.len());
                    bot.send_message(
                        chat_id,
                        "Invalid Solana public key: incorrect length. Must be a 32-byte key.",
                    )
                    .await?;
                    return Ok(());
                }
                Err(e) => {
                    println!("Failed to decode base58 public key: {}", e);
                    bot.send_message(
                        chat_id,
                        format!(
                "Invalid Solana public key format: {}. Must be a base58-encoded address.",
                e
            ),
                    )
                    .await?;
                    return Ok(());
                }
            };

            // Additional validation: try parsing as Pubkey
            if Pubkey::try_from(pubkey_bytes.as_slice()).is_err() {
                println!("Invalid Solana public key: not a valid Ed25519 key");
                bot.send_message(
                    msg.chat.id,
                    "Invalid Solana public key: not a valid Ed25519 key.",
                )
                .await?;
                return Ok(());
            }

            // Merge existing wallet data with new target_address
            let mut info_data = info
                .get(&chat_id.to_string())
                .cloned()
                .unwrap_or_else(|| json!({}));
            if let Some(obj) = info_data.as_object_mut() {
                obj.insert("target_address".to_string(), json!(&target_address));
            } else {
                info_data = json!({ "target_address": &target_address });
            }
            info[&chat_id.to_string()] = info_data;

            write_info(to_string(&info)?, None).await?;

            // Send success message with public key
            let response = format!("🎯 Target Address\n {}\n is correctly set", target_address);
            bot.send_message(msg.chat.id, response).await?;
        }
    }

    Ok(())
}

async fn run_trading(bot: Bot, dialogue: MyDialogue) -> HandlerResult {
    // Read info from data.json
    let info: Value = read_info(None).await?;

    // Check if chat ID already exists
    let chat_id = dialogue.chat_id();

    // Check if user info exists for this chat ID
    let user_info = info.get(chat_id.to_string());
    if let Some(user_data) = user_info {
        // Check for private_key
        if user_data.get("private_key").is_none() {
            bot.send_message(chat_id, "Your wallet has not been set up yet.")
                .reply_markup(setting_op_keyboard())
                .await?;
            dialogue.update(ChatState::SettingCb).await?;
            return Ok(());
        }

        // Check for target_address
        if user_data.get("target_address").is_none() {
            bot.send_message(chat_id, "The target address has not been configured yet.")
                .reply_markup(setting_op_keyboard())
                .await?;
            dialogue.update(ChatState::SettingCb).await?;
            return Ok(());
        }
    } else {
        // No user info exists for this chat ID
        bot.send_message(chat_id, "Can't find your id.")
            .await?;
        return Ok(());
    }

    // If we reach here, both private_key and target_address exist
    let response = "Run the trading".to_string();
    bot.send_message(chat_id, response).await?;

    copytrader_pumpfun(bot, chat_id).await?;

    Ok(())
}

async fn stop_trading(bot: Bot, _dialogue: MyDialogue, msg: Message) -> HandlerResult {
    let response = format!("stopped the trading");
    bot.send_message(msg.chat.id, response).await?;

    Ok(())
}

pub async fn send_msg(bot: Bot, chat_id: ChatId, prefix: String, msg: String) -> HandlerResult {
    Logger::new(prefix.clone()).log(msg.clone());
    bot.send_message(chat_id, strip_ansi_codes(&msg))
        .await
        .map_err(|e| Box::<dyn Error + Send + Sync>::from(e))?;
    Ok(())
}

fn strip_ansi_codes(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\x1B' {
            // ANSI escape code starts with ESC (0x1B)
            while let Some(next) = chars.next() {
                if next == 'm' {
                    // End of ANSI sequence
                    break;
                }
            }
        } else {
            result.push(ch);
        }
    }
    result
}
