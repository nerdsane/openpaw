//! Contract tests for OTS trajectory emission (ARN-109).
//!
//! These assert the wiring that decides whether a stored trajectory is usable
//! training data: that tool spans are actually persisted, that the emitter is
//! told which spec governed the run, and that the emitted JSON uses the exact
//! field names the kernel's `temper-ots` structs deserialize.

use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn session_spec() -> String {
    fs::read_to_string(repo_root().join("os-apps/paw-agent/specs/session.ioa.toml"))
        .expect("session.ioa.toml should exist")
}

fn emitter_source() -> String {
    fs::read_to_string(
        repo_root().join("os-apps/paw-agent/wasm/emit_ots_trajectory/src/ots_build.rs"),
    )
    .expect("ots_build.rs should exist")
}

/// The production defect this track fixes: with span persistence off, every
/// stored trajectory carried an empty `decisions` array.
#[test]
fn run_tools_persists_tool_spans() {
    let spec = session_spec();
    assert!(
        spec.contains("persist_tool_spans_file = \"true\""),
        "run_tools must persist tool spans; without them OTS decisions lose \
         tool wall-clock time and externalized turns lose their only evidence"
    );
    assert!(
        !spec.contains("persist_tool_spans_file = \"false\""),
        "no trigger may disable tool-span persistence"
    );
}

/// `metadata.spec_version` has to name the spec that actually ran, and the WASM
/// guest context exposes no spec hash — so the literal in the spec is the
/// identity, and it must track the app manifest.
#[test]
fn emitter_spec_version_matches_the_app_manifest() {
    let manifest = fs::read_to_string(repo_root().join("os-apps/paw-agent/app.toml"))
        .expect("paw-agent app.toml should exist");
    let version = manifest
        .lines()
        .find_map(|line| line.trim().strip_prefix("version = "))
        .map(|value| value.trim().trim_matches('"').to_string())
        .expect("app.toml should declare a version");
    let expected = format!("spec_version = \"paw-agent@{version}\"");

    assert!(
        session_spec().contains(&expected),
        "emit_ots_trajectory config must declare {expected}; the emitter reports \
         it as OTSMetadata.spec_version and it may not drift from app.toml"
    );
}

/// Every terminal path that finishes a session has to emit a trajectory —
/// otherwise the training set silently loses whole classes of run.
#[test]
fn every_terminal_action_emits_a_trajectory() {
    let spec = session_spec();
    let emit = "{ type = \"trigger\", name = \"emit_ots_trajectory\" }";
    for action in [
        "FinalizeResult",
        "FinalizeResultNoReply",
        "RecordResult",
        "RecordResultNoReply",
        "RecordResultInlineReply",
        "Fail",
        "Cancel",
        "TimeoutFail",
    ] {
        let marker = format!("name = \"{action}\"\n");
        let start = spec
            .find(&marker)
            .unwrap_or_else(|| panic!("{action} must exist in session.ioa.toml"));
        let block = &spec[start..];
        let block = &block[..block.find("\n[[action]]").unwrap_or(block.len())];
        assert!(
            block.contains(emit),
            "{action} must trigger emit_ots_trajectory"
        );
    }
}

/// Field names are the wire contract with the kernel's `temper-ots` structs.
/// A rename on either side silently drops the data at deserialization.
#[test]
fn emitter_uses_the_kernel_ots_field_names() {
    let source = emitter_source();
    for field in [
        // OTSTrajectory / OTSMetadata
        "\"trajectory_id\"",
        "\"version\"",
        "\"metadata\"",
        "\"turns\"",
        "\"task_description\"",
        "\"timestamp_start\"",
        "\"timestamp_end\"",
        "\"agent_id\"",
        "\"outcome\"",
        // ARN-109 additive metadata
        "\"harness\"",
        "\"spec_version\"",
        // OTSTurn
        "\"turn_id\"",
        "\"span_id\"",
        "\"timestamp\"",
        "\"messages\"",
        "\"decisions\"",
        // ARN-109 additive turn fields
        "\"prompt_token_ids\"",
        "\"completion_token_ids\"",
        "\"response_mask\"",
        "\"logprobs\"",
        // OTSMessage
        "\"message_id\"",
        "\"role\"",
        "\"content\"",
        "\"reasoning\"",
        // OTSDecision
        "\"decision_id\"",
        "\"decision_type\"",
        "\"cause_id\"",
        "\"choice\"",
        "\"consequence\"",
        "\"result_summary\"",
        "\"error_type\"",
    ] {
        assert!(
            source.contains(field),
            "emitter must produce the OTS field {field}"
        );
    }

    assert!(
        source.contains("\"tool_selection\""),
        "decision_type serializes snake_case per temper-ots enums"
    );
    assert!(
        source.contains("pub const HARNESS: &str = \"temperpaw\""),
        "metadata.harness identifies the runtime that produced the run"
    );
}

/// Inlining message bodies once cost ~300MB of a 491MB database (.proofs/061).
/// The budget constants are the guard, and the file-reference path is what
/// keeps large bodies out of the document entirely.
#[test]
fn emitter_bounds_inline_payloads() {
    let source = emitter_source();
    assert!(
        source.contains("MAX_MESSAGE_INLINE_CHARS"),
        "per-message inline ceiling must exist"
    );
    assert!(
        source.contains("MAX_TRAJECTORY_INLINE_CHARS"),
        "whole-document inline ceiling must exist"
    );
    assert!(
        source.contains("content_file_id"),
        "externalized bodies must be referenced by file id, never inlined"
    );
}

/// Retry idempotency: the Turso row is keyed on trajectory_id, so the id has to
/// be derived from the session and repeated inside metadata for the POST handler.
#[test]
fn emitter_keeps_trajectory_id_idempotency() {
    let lib = fs::read_to_string(
        repo_root().join("os-apps/paw-agent/wasm/emit_ots_trajectory/src/lib.rs"),
    )
    .expect("emit_ots_trajectory lib.rs should exist");
    assert!(
        lib.contains("format!(\"trj-{session_id}\")"),
        "trajectory_id must stay derived from the session id"
    );
    assert!(
        lib.contains("MarkTrajectoryEmitted") && lib.contains("TrajectoryEmissionFailed"),
        "emission status actions must stay wired"
    );
    assert!(
        emitter_source().contains("\"trajectory_id\": trajectory_id"),
        "metadata must repeat trajectory_id for the server-side POST handler"
    );

    let spec = session_spec();
    for action in [
        "MarkTrajectoryEmitted",
        "TrajectoryEmissionFailed",
        "RetryTrajectoryEmission",
    ] {
        assert!(
            spec.contains(&format!("name = \"{action}\"")),
            "{action} must stay declared on the Session automaton"
        );
    }
}

/// A trajectory is written once and marked emitted. Emitting one built from a
/// transcript the emitter could not read stores a permanently incomplete row
/// that retry will never repair, so an unreadable transcript has to fail the
/// emission instead of degrading it.
#[test]
fn emitter_fails_closed_when_the_transcript_cannot_be_read() {
    let lib = fs::read_to_string(
        repo_root().join("os-apps/paw-agent/wasm/emit_ots_trajectory/src/lib.rs"),
    )
    .expect("emit_ots_trajectory lib.rs should exist");

    let read_call = lib
        .find("match read_session_from_temperfs(")
        .expect("the emitter must read the session transcript");
    let error_arm = lib[read_call..]
        .find("Err(error) => {")
        .map(|offset| read_call + offset)
        .expect("the transcript read must handle its error case");
    let arm = &lib[error_arm..];
    let arm = &arm[..arm.find("\n        };").unwrap_or(arm.len())];

    assert!(
        arm.contains("TrajectoryEmissionFailed"),
        "an unreadable transcript must record a failed emission, not a partial trajectory"
    );
    assert!(
        arm.contains("return Ok(())"),
        "an unreadable transcript must stop before the POST"
    );
    assert!(
        !arm.contains("String::new()"),
        "substituting an empty transcript stores a spans-only row that is marked \
         emitted and therefore never repaired"
    );
}

/// Tool-call ids are unique only within a turn: providers that omit them get
/// synthetic ones that restart at every response. Anything keyed on them across
/// a session collapses two calls into one.
#[test]
fn tool_call_ids_survive_provider_fallbacks() {
    let wire = fs::read_to_string(
        repo_root().join("os-apps/paw-agent/wasm/openai-chat-wire/src/lib.rs"),
    )
    .expect("openai-chat-wire lib.rs should exist");
    assert!(
        wire.contains("pub fn synthetic_tool_call_id("),
        "the fallback id must be built in one place so every provider scopes it"
    );
    for source in [
        "os-apps/paw-agent/wasm/openai-chat-wire/src/lib.rs",
        "os-apps/paw-agent/wasm/provider_caller/src/lib.rs",
    ] {
        let text = fs::read_to_string(repo_root().join(source))
            .unwrap_or_else(|_| panic!("{source} should exist"));
        assert!(
            !text.contains("format!(\"tool_{}\", idx + 1)")
                && !text.contains("format!(\"or_tool_{}\", idx + 1)"),
            "{source} must not mint a turn-local tool call id"
        );
    }

    let emitter = emitter_source();
    assert!(
        emitter.contains("fn claim_span("),
        "the emitter must match spans to calls per turn, not through a \
         document-wide id index"
    );
    assert!(
        !emitter.contains("span_by_id"),
        "a document-wide id -> span map lets a later turn overwrite an earlier one"
    );
}

/// Serde ignores unknown fields, so deserializing the emitted document into the
/// kernel structs proves nothing about the fields the kernel does not model.
/// Those are enumerated and asserted separately.
#[test]
fn emitter_pins_the_fields_the_kernel_does_not_model() {
    let emitter = emitter_source();
    assert!(
        emitter.contains("KERNEL_UNMODELED_FIELDS"),
        "the extensions the pinned kernel drops on a round trip must be named"
    );
    assert!(
        emitter.contains("fn kernel_round_trip_drops_exactly_the_unmodeled_extensions"),
        "the unmodeled set must be asserted, so modeling one of them is noticed"
    );
    assert!(
        emitter.contains("fn rows_without_the_new_fields_still_deserialize"),
        "an old-row fixture must prove the additions stayed additive"
    );
    assert!(
        emitter.contains("HARNESS_TAG_PREFIX") && emitter.contains("SPEC_VERSION_TAG_PREFIX"),
        "run provenance must also travel in kernel-modeled metadata.tags"
    );
}

/// Per-turn timestamps and token counts come from the entry itself, because the
/// entity event log is a hot tail that drops older events at snapshot boundaries.
#[test]
fn session_entries_carry_their_own_wall_clock() {
    let helpers =
        fs::read_to_string(repo_root().join("os-apps/paw-agent/wasm/wasm-helpers/src/lib.rs"))
            .expect("wasm-helpers lib.rs should exist");
    assert!(
        helpers.contains("fn stamp_recorded_at"),
        "every SessionEntry must be stamped with its own creation time"
    );
    assert!(
        helpers.contains("\"ts_ms\""),
        "the stamp field is ts_ms; the emitter reads it back"
    );

    let applier = fs::read_to_string(
        repo_root().join("os-apps/paw-agent/wasm/provider_response_applier/src/lib.rs"),
    )
    .expect("provider_response_applier lib.rs should exist");
    assert!(
        applier.contains("fn assistant_turn_extra"),
        "assistant entries must record per-turn provider, model and usage facts"
    );
    for field in ["input_tokens", "output_tokens", "stop_reason"] {
        assert!(
            applier.contains(&format!("\"{field}\"")),
            "assistant turn extras must record {field}"
        );
    }
}
