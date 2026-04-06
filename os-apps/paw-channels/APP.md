# paw-channels

Multi-platform messaging adapter. Routes incoming messages from Discord, Slack, and webhooks to agents, and delivers replies back to the platform.

## Entity Types

### Channel
Connection lifecycle for a messaging platform.

- **States**: Created -> Connecting -> Connected <-> Disconnected -> Archived
- **Key actions**: `Configure` (channel_type, channel_id, guild_id), `Connect`, `ReceiveMessage`, `SendReply`, `Disconnect`, `Reconnect`
- **WASM**: `channel_connect` (establish connection), `route_message` (match message to agent), `send_reply` (deliver response)
- **Counters**: `active_sessions`, `message_count`

### ChannelSession
Maps channel threads to Agent entities for session continuity.

- **States**: Active -> Expired
- **Key actions**: `Create` (channel_id, thread_id, agent_entity_id), `Resume`, `Expire`

### AgentRoute
Binding-tier routing rules. Routes incoming messages to agents based on priority: peer > guild_roles > guild > team > channel.

- **States**: Active <-> Disabled
- **Key actions**: `Register` (binding_tier, channel_id, match_pattern, agent_config), `Update`, `Disable`, `Enable`

## Setup

Depends on `paw-agent` for agent identities and sessions. Configure Channel entities with platform credentials, then `Connect` to start receiving messages.
