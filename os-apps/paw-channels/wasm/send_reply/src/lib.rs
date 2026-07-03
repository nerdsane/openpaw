use temper_wasm_sdk::prelude::*;

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        let fields = ctx
            .entity_state
            .get("fields")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let delivery = prepare_delivery(&fields, &ctx.trigger_params)?;

        if !delivery.inline {
            let headers = vec![
                ("content-type".to_string(), "application/json".to_string()),
                ("x-tenant-id".to_string(), ctx.tenant.clone()),
            ];
            let resp = ctx.http_call(
                "POST",
                &delivery.webhook_url,
                &headers,
                &delivery.reply_params.to_string(),
            )?;
            if !(200..300).contains(&resp.status) {
                return Err(format!(
                    "send_reply: webhook POST failed (HTTP {})",
                    resp.status
                ));
            }
        }

        set_success_result("ReplyDelivered", &delivery.reply_params);
        Ok(())
    })();

    if let Err(error) = result {
        set_error_result(&error);
    }
    0
}

#[derive(Debug, PartialEq, Eq)]
struct Delivery {
    inline: bool,
    webhook_url: String,
    reply_params: Value,
}

fn prepare_delivery(fields: &Value, trigger_params: &Value) -> Result<Delivery, String> {
    let channel_type = str_field(fields, &["channel_type", "ChannelType"]).unwrap_or("");
    let webhook_url = str_field(fields, &["webhook_url", "WebhookUrl"])
        .unwrap_or("")
        .to_string();
    if webhook_url.is_empty() && !supports_inline_delivery(channel_type) {
        return Err("send_reply: webhook_url is empty".to_string());
    }

    Ok(Delivery {
        inline: webhook_url.is_empty(),
        webhook_url,
        reply_params: json!({
            "thread_id": action_param_or_field(trigger_params, fields, &["thread_id", "ThreadId"]).unwrap_or(""),
            "content": action_param_or_field(trigger_params, fields, &["content", "Content"]).unwrap_or(""),
            "agent_entity_id": action_param_or_field(trigger_params, fields, &["agent_entity_id", "AgentEntityId"]).unwrap_or(""),
            "reply_attachments_json": action_param(trigger_params, &["reply_attachments_json", "ReplyAttachmentsJson"]).unwrap_or(""),
        }),
    })
}

fn supports_inline_delivery(channel_type: &str) -> bool {
    matches!(channel_type, "cli" | "tui")
}

fn str_field<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
}

fn action_param<'a>(trigger_params: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| trigger_params.get(*key).and_then(Value::as_str))
}

fn action_param_or_field<'a>(
    trigger_params: &'a Value,
    fields: &'a Value,
    keys: &[&str],
) -> Option<&'a str> {
    action_param(trigger_params, keys).or_else(|| str_field(fields, keys))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_channel_without_webhook_delivers_inline() {
        let fields = json!({
            "channel_type": "cli",
            "thread_id": "main",
            "content": "hello",
            "agent_entity_id": "agent-1",
        });

        let delivery =
            prepare_delivery(&fields, &json!({})).expect("cli delivery should not need webhook");

        assert!(delivery.inline);
        assert_eq!(delivery.webhook_url, "");
        assert_eq!(delivery.reply_params["thread_id"], "main");
        assert_eq!(delivery.reply_params["content"], "hello");
        assert_eq!(delivery.reply_params["agent_entity_id"], "agent-1");
        assert_eq!(delivery.reply_params["reply_attachments_json"], "");
    }

    #[test]
    fn non_cli_channel_without_webhook_still_errors() {
        let fields = json!({
            "channel_type": "discord",
            "thread_id": "main",
            "content": "hello",
        });

        let error =
            prepare_delivery(&fields, &json!({})).expect_err("discord delivery needs a webhook");

        assert_eq!(error, "send_reply: webhook_url is empty");
    }

    #[test]
    fn delivery_prefers_current_action_params_over_stale_channel_fields() {
        let fields = json!({
            "channel_type": "discord",
            "webhook_url": "http://127.0.0.1/reply",
            "thread_id": "old-thread",
            "content": "old content",
            "agent_entity_id": "old-agent",
            "reply_attachments_json": ""
        });
        let trigger_params = json!({
            "thread_id": "new-thread",
            "content": "new content",
            "agent_entity_id": "new-agent",
            "reply_attachments_json": "[{\"kind\":\"pawfs_file\",\"file_id\":\"fl-1\"}]"
        });

        let delivery = prepare_delivery(&fields, &trigger_params)
            .expect("discord delivery should use current action params");

        assert!(!delivery.inline);
        assert_eq!(delivery.reply_params["thread_id"], "new-thread");
        assert_eq!(delivery.reply_params["content"], "new content");
        assert_eq!(delivery.reply_params["agent_entity_id"], "new-agent");
        assert_eq!(
            delivery.reply_params["reply_attachments_json"],
            "[{\"kind\":\"pawfs_file\",\"file_id\":\"fl-1\"}]"
        );
    }

    #[test]
    fn missing_current_attachment_param_does_not_reuse_stale_channel_attachment() {
        let fields = json!({
            "channel_type": "discord",
            "webhook_url": "http://127.0.0.1/reply",
            "thread_id": "old-thread",
            "content": "old content",
            "agent_entity_id": "old-agent",
            "reply_attachments_json": "[{\"kind\":\"pawfs_file\",\"file_id\":\"stale-cat\"}]"
        });
        let trigger_params = json!({
            "thread_id": "new-thread",
            "content": "new content",
            "agent_entity_id": "new-agent"
        });

        let delivery = prepare_delivery(&fields, &trigger_params)
            .expect("discord delivery should use current action params");

        assert_eq!(delivery.reply_params["thread_id"], "new-thread");
        assert_eq!(delivery.reply_params["content"], "new content");
        assert_eq!(delivery.reply_params["agent_entity_id"], "new-agent");
        assert_eq!(delivery.reply_params["reply_attachments_json"], "");
    }
}
