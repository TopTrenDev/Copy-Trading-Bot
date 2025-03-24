use solana_sdk::{bs58, signature::Keypair, signer::Signer};
use std::collections::HashMap;
use std::error::Error;
use teloxide::{
    dispatching::{
        dialogue::{self, InMemStorage},
        UpdateHandler,
    },
    prelude::*,
    types::{KeyboardMarkup, MaybeInaccessibleMessage},
    utils::{command::BotCommands, html::escape},
};

use crate::msg::{start_op_keyboard, StartOp};

type MyDialogue = Dialogue<ChatState, InMemStorage<ChatState>>;
type HandlerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone, Default)]
enum ChatState {
    #[default]
    Start,
    StartCb,
    WalletInput,
    Wallet {
        private_key: String,
    },
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
        .branch(case![ChatState::Start].branch(case![Command::Start].endpoint(start)))
        .branch(case![Command::Help].endpoint(help))
        .branch(case![Command::Stop].endpoint(cancel))
        .branch(dptree::endpoint(invalid_command));

    // Text
    #[rustfmt::skip]
    let text_handler = Message::filter_text()
        .branch(case![ChatState::WalletInput].endpoint(add_wallet));
    // .branch(case![State::FindSupervisor].endpoint(find_supervisor));

    // Information
    let message_handler = Update::filter_message()
        .branch(command_handler)
        .branch(text_handler)
        // .branch(case![ChatState::ReceiveTokenAddress { chain_id }].endpoint(receive_token_address))
        // .branch(
        //     case![ChatState::ReceiveUser {
        //         chain_id,
        //         token_address
        //     }]
        //     .endpoint(receive_user),
        // )
        .branch(dptree::endpoint(invalid_state));

    let callback_query_handler = Update::filter_callback_query()
        // .branch(case![ChatState::ReceiveChainId].endpoint(receive_chain_id))
        .branch(case![ChatState::StartCb].endpoint(start_cb))
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
    // bot.send_message(msg.chat.id, "Let's start! Select desired chain.")
    //     .await?;
    bot.send_message(msg.chat.id, "Welcome to copytrading bot with Rust")
        .reply_markup(start_op_keyboard())
        .await?;
    dialogue.update(ChatState::StartCb).await?;
    Ok(())
}

async fn start_cb(bot: Bot, dialogue: MyDialogue, q: CallbackQuery) -> HandlerResult {
    bot.answer_callback_query(q.id).await?; // todo 先别 await
    if let Some(op) = &q.data {
        match serde_json::from_str(op)? {
            StartOp::Wallet => {
                let text = "Please input Your wallet\n";
                bot.send_message(dialogue.chat_id(), text).await?;
                dialogue.update(ChatState::WalletInput).await?;
            }
            StartOp::Target => {
                let text = "Wallet Target\n\
                    Whale, Trader\n";
                bot.send_message(dialogue.chat_id(), text).await?;
                // dialogue.update(State::FindSupervisor).await?;
            }
            StartOp::Help => {}
        }
    }
    Ok(())
}

async fn add_wallet(bot: Bot, dialogue: MyDialogue, msg: Message) -> HandlerResult {
    if let Some(private_key) = msg.text().map(ToOwned::to_owned) {
        let keypair_bytes = match bs58::decode(private_key).into_vec() {
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
        // Send success message with public key
        let response = format!(
            "👛Your wallet\n {}\n is correctly imported",
            wallet.pubkey()
        );
        bot.send_message(msg.chat.id, response).await?;

        // dialogue.update(ChatState::Wallet { private_key: &private_key }).await?;
    } else {
        bot.send_message(msg.chat.id, "Empty message error. Sorry, please try again")
            .await?;
    }
    Ok(())
}

// async fn add_wallet_msg(private_key: &String, bot: &Bot, msg: &Message) -> HandlerResult {
//     let keypair_bytes = match bs58::decode(private_key).into_vec() {
//         Ok(bytes) => bytes,
//         Err(e) => {
//             println!("Failed to decode base58 private key: {}", e);
//             bot.send_message(
//                 msg.chat.id,
//                 format!("Invalid private key. {} Please try again.", e),
//             )
//             .await?;
//             return Ok(()); // Exit after notifying user
//         }
//     };

//     // Create Keypair from bytes
//     let wallet = match Keypair::from_bytes(&keypair_bytes) {
//         Ok(kp) => kp,
//         Err(e) => {
//             println!("Invalid private key bytes: {}", e);
//             bot.send_message(
//                 msg.chat.id,
//                 format!("Invalid private key. {} Please try again.", e),
//             )
//             .await?;
//             return Ok(()); // Exit after notifying user
//         }
//     };
//     // Send success message with public key
//     let response = format!("🧭 {}\nkey：", wallet.pubkey());
//     bot.send_message(msg.chat.id, response).await?;

//     Ok(())
// }
