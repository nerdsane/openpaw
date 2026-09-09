//! Explicit live provider proof; run only on the source Computer.
use serde_json::json;
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, RwLock},
    time::Duration,
};
use temper_wasm::{
    ProductionWasmHost, StreamRegistry, WasmEngine, WasmInvocationContext, WasmResourceLimits,
};

#[tokio::test(flavor = "multi_thread")]
#[ignore = "creates and deletes one real Tensorlake copy; requires explicit source and credential file"]
async fn copied_computer_preserves_source_file() -> anyhow::Result<()> {
    let source = std::env::var("ARN467_SOURCE_ID")?;
    let credential: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
        std::env::var("ARN467_TL_KEY_FILE")?,
    )?)?;
    let key = credential["token"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing token"))?
        .to_owned();
    let mut scope = reqwest::header::HeaderMap::new();
    for (name, field) in [
        ("X-Forwarded-Organization-Id", "organization"),
        ("X-Forwarded-Project-Id", "project"),
    ] {
        scope.insert(
            reqwest::header::HeaderName::from_bytes(name.as_bytes())?,
            credential[field]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("missing project scope"))?
                .parse()?,
        );
    }
    let client = reqwest::Client::builder()
        .default_headers(scope)
        .timeout(Duration::from_secs(120))
        .build()?;
    let resume = std::env::var("ARN467_COPY_CHILD_ID").ok();
    let child = resume
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let marker_path = format!("/tmp/arn467-copy-marker-{child}");
    let marker = format!("ARN-467 source={source} child={child}");
    std::fs::write(&marker_path, &marker)?;
    let engine = WasmEngine::new()?;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../os-apps/paw-compute/wasm");
    let mut modules = BTreeMap::new();
    for module in [
        "computer_copy_start",
        "computer_copy_poll",
        "computer_terminate",
    ] {
        let bytes = std::fs::read(root.join(module).join(format!("{module}.wasm")))?;
        modules.insert(module, engine.compile_and_cache(&bytes)?);
    }
    // Use the existing personal credential's project scope. Every request still
    // reaches the real provider; no responses or callbacks are simulated.
    let provider_client = client.clone();
    let host = Arc::new(
        ProductionWasmHost::new(BTreeMap::new()).with_text_http_interceptor(Arc::new(
            move |method, url, headers, body| {
                let client = provider_client.clone();
                Box::pin(async move {
                    let result = async {
                        let mut request =
                            client.request(method.parse().map_err(|e| format!("{e}"))?, &url);
                        for (name, value) in headers {
                            request = request.header(name, value);
                        }
                        let response =
                            request.body(body).send().await.map_err(|e| e.to_string())?;
                        let status = response.status().as_u16();
                        let text = response.text().await.map_err(|e| e.to_string())?;
                        Ok((status, text))
                    }
                    .await;
                    Some(result)
                })
            },
        )),
    );
    let streams = Arc::new(RwLock::new(StreamRegistry::default()));
    let limits = WasmResourceLimits::default();
    let mut context = WasmInvocationContext {
        tenant: "default".into(),
        entity_type: "Computer".into(),
        entity_id: child.clone(),
        trigger_action: "ProvisionFromCopy".into(),
        wasm_module: Some("computer_copy_start".into()),
        trigger_params: json!({}),
        entity_state: json!({"status":"Provisioning","fields":{"machine_id":source,"provider":"tensorlake"}}),
        agent_id: None,
        session_id: None,
        integration_config: BTreeMap::from([("tensorlake_api_key".into(), key.clone())]),
        trace_id: String::new(),
        workflow_root_entity_type: None,
        workflow_root_entity_id: None,
        workflow_run_id: None,
        http_request: None,
    };
    if resume.is_some() {
        context.trigger_action = "ReconcileCopy".into();
        context.entity_state["status"] = json!("CopyUnknown");
    }
    println!(
        "COPY_REQUEST source={source} child={child} action={}",
        context.trigger_action
    );
    let mut started = engine
        .invoke(
            &modules["computer_copy_start"],
            &context,
            host.clone(),
            &limits,
            streams.clone(),
        )
        .await?;
    let reconcile_deadline = tokio::time::Instant::now() + Duration::from_secs(300);
    while !started.success {
        println!(
            "COPY_UNCERTAIN child={child} error={:?}; subsequent requests are GET-only",
            started.error
        );
        anyhow::ensure!(
            tokio::time::Instant::now() < reconcile_deadline,
            "copy remains uncertain"
        );
        context.trigger_action = "ReconcileCopy".into();
        context.entity_state["status"] = json!("CopyUnknown");
        tokio::time::sleep(Duration::from_secs(5)).await;
        started = engine
            .invoke(
                &modules["computer_copy_start"],
                &context,
                host.clone(),
                &limits,
                streams.clone(),
            )
            .await?;
    }
    anyhow::ensure!(
        started.callback_action == "CopyStarted",
        "unexpected start callback"
    );
    let destination = started.callback_params["machine_id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing destination"))?
        .to_owned();
    anyhow::ensure!(
        !destination.is_empty() && destination != source,
        "copy must differ from source"
    );
    anyhow::ensure!(
        started.callback_params["source_machine_id"] == source,
        "source binding changed"
    );
    println!("COPY_CREATED source={source} destination={destination} child={child}");
    let mut fields = started.callback_params.clone();
    fields["provider"] = json!("tensorlake");
    context.entity_state = json!({"status":"Copying","fields":fields});
    context.trigger_action = "CopyPoll".into();
    context.wasm_module = Some("computer_copy_poll".into());
    let proof = async {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(300);
        loop {
            let result = engine
                .invoke(
                    &modules["computer_copy_poll"],
                    &context,
                    host.clone(),
                    &limits,
                    streams.clone(),
                )
                .await?;
            anyhow::ensure!(result.success, "copy readiness: {:?}", result.error);
            if result.callback_action == "CopyComplete" {
                break;
            }
            anyhow::ensure!(
                tokio::time::Instant::now() < deadline,
                "copy readiness deadline"
            );
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
        let sandbox_url = fields["sandbox_url"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing sandbox URL"))?;
        let read = client
            .get(format!("{sandbox_url}/api/v1/files"))
            .query(&[("path", &marker_path)])
            .bearer_auth(&key)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        anyhow::ensure!(read == marker, "copied file did not match source");
        let source_row: serde_json::Value = client
            .get(format!("https://api.tensorlake.ai/sandboxes/{source}"))
            .bearer_auth(&key)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        anyhow::ensure!(source_row["id"] == source, "source disappeared");
        println!(
            "COPY_FILE_VERIFIED source={source} destination={destination} bytes={}",
            read.len()
        );
        Ok::<(), anyhow::Error>(())
    }
    .await;
    context.trigger_action = "Destroy".into();
    context.wasm_module = Some("computer_terminate".into());
    context.entity_state = json!({"status":"Terminating","fields":fields});
    let cleanup = engine
        .invoke(
            &modules["computer_terminate"],
            &context,
            host,
            &limits,
            streams,
        )
        .await?;
    anyhow::ensure!(cleanup.success, "copy cleanup failed");
    std::fs::remove_file(marker_path)?;
    let cleanup_deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let response = client
            .get(format!("https://api.tensorlake.ai/sandboxes/{destination}"))
            .bearer_auth(&key)
            .send()
            .await?;
        let status = response.status();
        let body: serde_json::Value = response.json().await.unwrap_or_default();
        if status.as_u16() == 404 || body["status"] == "terminated" {
            break;
        }
        anyhow::ensure!(
            tokio::time::Instant::now() < cleanup_deadline,
            "copy remains after cleanup: HTTP {status}"
        );
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    println!("COPY_CLEANUP_VERIFIED destination={destination}");
    proof
}
