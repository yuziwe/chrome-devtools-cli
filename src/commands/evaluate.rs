use anyhow::Result;
use serde_json::json;

use crate::cdp::CdpClient;

pub async fn evaluate(
    client: &mut CdpClient,
    session_id: &str,
    expression: &str,
    as_json: bool,
    dialog_action: Option<&str>,
) -> Result<String> {
    if dialog_action.is_some() {
        client
            .send_to_target(session_id, "Page.enable", json!({}))
            .await?;
    }

    // Prepare the evaluation command
    let id = client
        .send_raw_no_wait(
            Some(session_id),
            "Runtime.evaluate",
            json!({
                "expression": expression,
                "returnByValue": true,
                "awaitPromise": true,
            }),
        )
        .await?;

    // Wait for the response, but also handle Page.javascriptDialogOpening events
    loop {
        let resp_text = client.read_text().await?;
        let resp: serde_json::Value = serde_json::from_str(&resp_text)?;

        if resp.get("id").and_then(|v| v.as_u64()) == Some(id) {
            if let Some(error) = resp.get("error") {
                anyhow::bail!(
                    "CDP error in Runtime.evaluate: {}",
                    serde_json::to_string_pretty(error)?
                );
            }
            let result = &resp["result"];
            if let Some(exception) = result.get("exceptionDetails") {
                let text = exception["text"].as_str().unwrap_or("Unknown error");
                let desc = exception["exception"]["description"]
                    .as_str()
                    .unwrap_or(text);
                anyhow::bail!("{desc}");
            }

            let value = &result["result"];
            let val_type = value["type"].as_str().unwrap_or("undefined");

            if as_json {
                if let Some(v) = value.get("value") {
                    return Ok(serde_json::to_string_pretty(v)?);
                } else {
                    return Ok(serde_json::to_string_pretty(value)?);
                }
            } else {
                match val_type {
                    "undefined" => return Ok("undefined".to_string()),
                    "string" => return Ok(value["value"].as_str().unwrap_or("").to_string()),
                    _ => {
                        if let Some(v) = value.get("value") {
                            return Ok(serde_json::to_string_pretty(v)?);
                        } else {
                            return Ok(value["description"].as_str().unwrap_or("").to_string());
                        }
                    }
                }
            }
        }

        // Handle dialog events if they occur
        if resp.get("method").and_then(|v| v.as_str()) == Some("Page.javascriptDialogOpening") {
            if let Some(action) = dialog_action {
                let mut params = json!({});
                match action {
                    "accept" => {
                        params["accept"] = json!(true);
                    }
                    "dismiss" => {
                        params["accept"] = json!(false);
                    }
                    text => {
                        params["accept"] = json!(true);
                        params["promptText"] = json!(text);
                    }
                }
                client
                    .send_to_target(session_id, "Page.handleJavaScriptDialog", params)
                    .await?;
            }
        }
    }
}
