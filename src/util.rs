use reqwest::blocking::Client;
use serde_json::json;

use crate::app::ui::edtior_info::{EDITOR_STAGE, EDITOR_VERSION};

pub mod expression_parser;
pub mod system_stats;
pub mod timer;

pub fn send_discord_webhook_crash_message(webhook_url: &str, content: &str, api_key: &str) -> Result<(), reqwest::Error> {
    let client = Client::new();

    client
        .post(webhook_url)
        .header("x-api-key", api_key)
        .json(&json!({
            "embeds": [{
                "title": "Crash Report",
                "description": format!("**Version:** {}-{}", EDITOR_VERSION, EDITOR_STAGE),
                "color": 0xFF0000,
                "fields": [
                    {
                        "name": "Error Description",
                        "value": format!("```{}```", content),
                        "inline": false
                    }
                ]
            }]
        }))
        .send()?
        .error_for_status()?;

    Ok(())
}