//! workspace_fs — WASM module for FUSE-mapped filesystem operations on Workspace entities.
//!
//! Each integration trigger dispatches to a single filesystem operation:
//! mkdir, create_file, resolve_path, list_dir, delete_file.
//!
//! The `operation` config field selects which function to run.
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use temper_wasm_sdk::prelude::*;

mod ops;
mod path;

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;

        let api_url = ctx
            .config
            .get("temper_api_url")
            .ok_or("workspace_fs: missing temper_api_url config")?
            .clone();
        let operation = ctx
            .config
            .get("operation")
            .ok_or("workspace_fs: missing operation config")?
            .clone();
        let tenant = ctx.tenant.clone();
        let ws_id = ctx.entity_id.clone();

        let raw_path = ctx
            .trigger_params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or("workspace_fs: missing path parameter")?;

        ctx.log(
            "info",
            &format!("workspace_fs: op={operation} path={raw_path} ws={ws_id}"),
        );

        match operation.as_str() {
            "mkdir" => ops::mkdir(&ctx, &api_url, &tenant, &ws_id, raw_path),
            "create_file" => {
                let mime_type = ctx
                    .trigger_params
                    .get("mime_type")
                    .and_then(|v| v.as_str());
                ops::create_file(&ctx, &api_url, &tenant, &ws_id, raw_path, mime_type)
            }
            "resolve_path" => ops::resolve_path(&ctx, &api_url, &tenant, &ws_id, raw_path),
            "list_dir" => ops::list_dir(&ctx, &api_url, &tenant, &ws_id, raw_path),
            "delete_file" => ops::delete_file(&ctx, &api_url, &tenant, &ws_id, raw_path),
            other => Err(format!("workspace_fs: unknown operation: {other}")),
        }
    })();

    match result {
        Ok(()) => 0,
        Err(e) => {
            set_error_result(&e);
            1
        }
    }
}
