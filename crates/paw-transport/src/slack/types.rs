//! Slack Socket Mode and Web API types.
//!
//! Covers the subset of the Slack API needed for receiving messages via
//! Socket Mode and sending replies via the Web API.

use serde::{Deserialize, Serialize};

// ── Socket Mode envelope ────────────────────────────────────────────

/// Top-level envelope received over the Socket Mode WebSocket.
///
/// Every envelope must be acknowledged by sending `{"envelope_id": "..."}` back
/// within 3 seconds.
#[derive(Debug, Deserialize)]
pub struct SlackEnvelope {
    /// Unique ID for this envelope — must be echoed back as acknowledgment.
    pub envelope_id: String,
    /// Envelope type: "events_api", "interactive", "slash_commands".
    #[serde(rename = "type")]
    pub envelope_type: String,
    /// The actual event or interaction payload.
    pub payload: serde_json::Value,
    /// Whether the acknowledgment can include a response payload.
    #[serde(default)]
    pub accepts_response_payload: bool,
}

/// Acknowledgment sent back over the WebSocket for each envelope.
#[derive(Debug, Serialize)]
pub struct EnvelopeAck {
    pub envelope_id: String,
}

// ── Events API payload ──────────────────────────────────────────────

/// Wrapper for Events API payloads delivered via Socket Mode.
#[derive(Debug, Deserialize)]
pub struct EventsApiPayload {
    /// The inner event object.
    pub event: serde_json::Value,
    /// Event type at the top level (e.g., "event_callback").
    #[serde(rename = "type")]
    pub payload_type: Option<String>,
    /// Event ID for dedup.
    pub event_id: Option<String>,
}

/// A Slack message event (event.type = "message").
#[derive(Debug, Deserialize)]
pub struct SlackMessageEvent {
    /// User ID who sent the message.
    pub user: Option<String>,
    /// Message text content.
    pub text: Option<String>,
    /// Channel ID (DM channel for IMs).
    pub channel: Option<String>,
    /// Message timestamp (serves as unique message ID in Slack).
    pub ts: Option<String>,
    /// Channel type: "im" for DMs, "channel" for public, "group" for private.
    pub channel_type: Option<String>,
    /// Event subtype (None for normal user messages, "bot_message" for bots, etc.).
    pub subtype: Option<String>,
    /// Bot ID if this is a bot message.
    pub bot_id: Option<String>,
    /// Inner event type (should be "message").
    #[serde(rename = "type")]
    pub event_type: Option<String>,
}

// ── Interactive payloads (button clicks) ────────────────────────────

/// Interaction payload for block_actions (button clicks).
#[derive(Debug, Deserialize)]
pub struct SlackInteractionPayload {
    /// Interaction type: "block_actions", "message_actions", etc.
    #[serde(rename = "type")]
    pub interaction_type: String,
    /// The user who triggered the interaction.
    pub user: SlackUser,
    /// Actions the user took (button clicks).
    #[serde(default)]
    pub actions: Vec<SlackAction>,
    /// Channel where the interaction occurred.
    pub channel: Option<SlackChannel>,
    /// The original message that contained the interactive elements.
    pub message: Option<serde_json::Value>,
    /// URL for responding to this interaction (3-second window).
    pub response_url: Option<String>,
    /// Trigger ID for opening modals.
    pub trigger_id: Option<String>,
}

/// Minimal Slack user object.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SlackUser {
    pub id: String,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
}

/// A user action (button click).
#[derive(Debug, Deserialize)]
pub struct SlackAction {
    /// Action ID set when the block was created (e.g., "approve:decision-123").
    pub action_id: String,
    /// Block ID containing this action.
    #[serde(default)]
    pub block_id: Option<String>,
    /// Action type (e.g., "button").
    #[serde(rename = "type")]
    pub action_type: String,
    /// Action timestamp.
    #[serde(default)]
    pub action_ts: Option<String>,
}

/// Minimal Slack channel object.
#[derive(Debug, Deserialize)]
pub struct SlackChannel {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
}

// ── Block Kit types ─────────────────────────────────────────────────

/// A Block Kit block element.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SlackBlock {
    /// A section block with text.
    #[serde(rename = "section")]
    Section {
        text: SlackTextObject,
        #[serde(skip_serializing_if = "Option::is_none")]
        block_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        fields: Option<Vec<SlackTextObject>>,
    },
    /// An actions block containing interactive elements.
    #[serde(rename = "actions")]
    Actions {
        elements: Vec<SlackButtonElement>,
        #[serde(skip_serializing_if = "Option::is_none")]
        block_id: Option<String>,
    },
    /// A divider line.
    #[serde(rename = "divider")]
    Divider {
        #[serde(skip_serializing_if = "Option::is_none")]
        block_id: Option<String>,
    },
    /// A context block with small text/images.
    #[serde(rename = "context")]
    Context {
        elements: Vec<SlackTextObject>,
        #[serde(skip_serializing_if = "Option::is_none")]
        block_id: Option<String>,
    },
}

/// A text object used in blocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackTextObject {
    /// "mrkdwn" or "plain_text".
    #[serde(rename = "type")]
    pub text_type: String,
    pub text: String,
}

impl SlackTextObject {
    /// Create a mrkdwn text object.
    pub fn mrkdwn(text: &str) -> Self {
        Self {
            text_type: "mrkdwn".to_string(),
            text: text.to_string(),
        }
    }

    /// Create a plain_text text object.
    pub fn plain_text(text: &str) -> Self {
        Self {
            text_type: "plain_text".to_string(),
            text: text.to_string(),
        }
    }
}

/// A button element for actions blocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackButtonElement {
    /// Always "button".
    #[serde(rename = "type")]
    pub element_type: String,
    /// Button label.
    pub text: SlackTextObject,
    /// Action ID — parsed as "approve:{decision_id}" or "deny:{decision_id}".
    pub action_id: String,
    /// Button style: "primary" (green) or "danger" (red). Omit for default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
}

impl SlackButtonElement {
    /// Create an "Approve" button.
    pub fn approve(decision_id: &str) -> Self {
        Self {
            element_type: "button".to_string(),
            text: SlackTextObject::plain_text("Approve"),
            action_id: format!("approve:{decision_id}"),
            style: Some("primary".to_string()),
        }
    }

    /// Create a "Deny" button.
    pub fn deny(decision_id: &str) -> Self {
        Self {
            element_type: "button".to_string(),
            text: SlackTextObject::plain_text("Deny"),
            action_id: format!("deny:{decision_id}"),
            style: Some("danger".to_string()),
        }
    }
}

/// Helper to build approval blocks (section + buttons).
pub fn approval_blocks(decision_id: &str, description: &str) -> Vec<SlackBlock> {
    vec![
        SlackBlock::Section {
            text: SlackTextObject::mrkdwn(description),
            block_id: None,
            fields: None,
        },
        SlackBlock::Actions {
            elements: vec![
                SlackButtonElement::approve(decision_id),
                SlackButtonElement::deny(decision_id),
            ],
            block_id: Some(format!("decision_{decision_id}")),
        },
    ]
}

// ── REST API response types ─────────────────────────────────────────

/// Response from Slack's `auth.test` method.
#[derive(Debug, Deserialize)]
pub struct AuthTestResponse {
    pub ok: bool,
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub user: Option<String>,
    #[serde(default)]
    pub team_id: Option<String>,
    #[serde(default)]
    pub team: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

/// Response from `apps.connections.open`.
#[derive(Debug, Deserialize)]
pub struct ConnectionsOpenResponse {
    pub ok: bool,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

/// Generic Slack API response (for chat.postMessage, chat.update, etc.).
#[derive(Debug, Deserialize)]
pub struct SlackApiResponse {
    pub ok: bool,
    #[serde(default)]
    pub error: Option<String>,
    /// Channel the message was posted to.
    #[serde(default)]
    pub channel: Option<String>,
    /// Timestamp of the posted/updated message.
    #[serde(default)]
    pub ts: Option<String>,
    /// The full message object.
    #[serde(default)]
    pub message: Option<serde_json::Value>,
}
