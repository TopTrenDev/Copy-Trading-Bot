use serde::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};
use url::Url;

#[derive(Serialize, Deserialize, Debug)]
pub enum StartOp {
    Wallet,
    Target,
    Help,
}

impl From<StartOp> for String {
    fn from(val: StartOp) -> Self {
        serde_json::to_string(&val).unwrap()
    }
}

pub fn start_op_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new([
        vec![InlineKeyboardButton::callback("👝 Wallet", StartOp::Wallet)],
        vec![InlineKeyboardButton::callback("🎯 Target", StartOp::Target)],
        vec![InlineKeyboardButton::callback("ℹ️ Help", StartOp::Help)],
        vec![InlineKeyboardButton::url(
            "^_^ Source",
            Url::parse("https://github.com/sinniez/Solana--Copytrading-Tool--Rust").unwrap(),
        )],
    ])
}
