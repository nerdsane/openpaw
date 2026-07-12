use std::io;
use std::sync::{Arc, Mutex};

use tracing_subscriber::fmt::MakeWriter;

use super::*;

#[derive(Clone, Default)]
struct SharedWriter {
    buffer: Arc<Mutex<Vec<u8>>>,
}

impl SharedWriter {
    fn output(&self) -> String {
        String::from_utf8(self.buffer.lock().unwrap().clone()).unwrap_or_default()
    }
}

struct SharedLogGuard {
    buffer: Arc<Mutex<Vec<u8>>>,
}

impl io::Write for SharedLogGuard {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buffer.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for SharedWriter {
    type Writer = SharedLogGuard;

    fn make_writer(&'a self) -> Self::Writer {
        SharedLogGuard {
            buffer: self.buffer.clone(),
        }
    }
}

#[test]
fn webhook_logging_uses_structured_tracing_without_payload_body() {
    let writer = SharedWriter::default();
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .without_time()
        .with_writer(writer.clone())
        .finish();

    tracing::subscriber::with_default(subscriber, || {
        log_webhook_event(WebhookEventLog {
            operation: "receive",
            outcome: "success",
            route_key: "github",
            event_id: "wh-123",
            status: 200,
            payload_bytes: "secret webhook body".len(),
            error: "",
        });
    });

    let output = writer.output();
    assert!(output.contains("observability_event=\"temperpaw.webhook\""));
    assert!(output.contains("webhook.route_key=\"github\""));
    assert!(output.contains("webhook.event_id=\"wh-123\""));
    assert!(output.contains("webhook.status=200"));
    assert!(
        !output.contains("secret webhook body"),
        "webhook logs must not emit payload bodies, got: {output:?}"
    );
}
