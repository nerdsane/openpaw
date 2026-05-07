#[test]
fn boot_poll_scans_newest_queue_window_not_old_stale_runs() {
    let source = include_str!("boot_watch.rs");

    assert!(
        source.contains("$orderby=Id desc&$top=50"),
        "boot poll should inspect newest queued entities first so old stale runs cannot starve newer claimable runs"
    );
}

#[test]
fn event_stream_triggers_worker_queue_fallback_after_each_entity_event() {
    let source = include_str!("event_loop.rs");

    assert!(
        source.contains("claim_boot_queued_runs(client, config).await?"),
        "SSE handling should immediately rescan queued WorkerRuns after entity events so missed WorkerRun events cannot wait behind stale stream replay"
    );
}

#[test]
fn event_stream_polls_queue_while_connection_stays_open() {
    let source = include_str!("event_loop.rs");

    assert!(
        source.contains("event_stream_queue_poll_interval()"),
        "an open SSE stream with heartbeats must still poll the queued work window"
    );
    assert!(
        source.contains("claim_event_stream_backlog(client, config).await?"),
        "periodic stream fallback should process WorkerRun, ReviewRun, and EvaluationRun backlogs"
    );
}
