use serde::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};
use url::Url;

#[derive(Serialize, Deserialize, Debug)]
pub enum StartOp {
    Run,
    Stop,
    Setting,
    Help,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum SettingOp {
    Wallet,
    Target,
}

impl From<StartOp> for String {
    fn from(val: StartOp) -> Self {
        serde_json::to_string(&val).unwrap()
    }
}

impl From<SettingOp> for String {
    fn from(val: SettingOp) -> Self {
        serde_json::to_string(&val).unwrap()
    }
}

pub fn start_op_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new([
        vec![InlineKeyboardButton::callback("🟢 Run", StartOp::Run)],
        vec![InlineKeyboardButton::callback("🛑 Stop", StartOp::Stop)],
        vec![InlineKeyboardButton::callback(
            "⚙️ Setting",
            StartOp::Setting,
        )],
        vec![InlineKeyboardButton::callback("ℹ️ Help", StartOp::Help)],
        vec![InlineKeyboardButton::url(
            "^_^ Source",
            Url::parse("https://github.com/sinniez/Solana--Copytrading-Tool--Rust").unwrap(),
        )],
    ])
}

pub fn setting_op_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new([
        vec![InlineKeyboardButton::callback(
            "👝 Wallet",
            SettingOp::Wallet,
        )],
        vec![InlineKeyboardButton::callback(
            "🎯 Target",
            SettingOp::Target,
        )],
    ])
}
