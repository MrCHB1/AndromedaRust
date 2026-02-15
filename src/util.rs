use reqwest::blocking::Client;
use serde_json::json;

use crate::app::ui::edtior_info::{EDITOR_STAGE, EDITOR_VERSION};

pub mod expression_parser;
pub mod system_stats;
pub mod timer;
pub mod debugger;

pub fn send_discord_webhook_crash_message(webhook_url: &str, content: &str, api_key: &str, reporter_name: Option<String>, report_details: Option<String>) -> Result<(), reqwest::Error> {
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
                        "name": "Reporter name",
                        "value": if let Some(name) = reporter_name { name } else { "Anonymous".into() },
                        "inline": false
                    },
                    {
                        "name": "Cause of crash",
                        "value": if let Some(details) = report_details { details } else { "No further details given.".into() },
                        "inline": false
                    },
                    {
                        "name": "Crash details",
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