use std::borrow::Cow;

use monty::{MontyException, PrintWriterCallback};

pub(crate) const MAX_TOOL_RESULT_BYTES: usize = 16 * 1024;

/// Captures `print()` output without allowing Monty to grow an unbounded host-side buffer.
///
/// The original implementation used `PrintWriter::Collect(String)` and only truncated after
/// execution completed. Large or pathological output therefore forced repeated reallocations
/// inside the daemon and could balloon RSS before we ever returned a tool result.
pub(crate) struct BoundedOutputCollector {
    buf: String,
    max_bytes: usize,
    total_bytes_seen: usize,
    truncated: bool,
}

impl BoundedOutputCollector {
    pub(crate) fn new(max_bytes: usize) -> Self {
        Self {
            buf: String::with_capacity(max_bytes.min(1024)),
            max_bytes,
            total_bytes_seen: 0,
            truncated: false,
        }
    }

    fn append_str(&mut self, output: &str) {
        self.total_bytes_seen += output.len();
        if self.buf.len() >= self.max_bytes {
            self.truncated = true;
            return;
        }

        let remaining = self.max_bytes - self.buf.len();
        if output.len() <= remaining {
            self.buf.push_str(output);
            return;
        }

        self.truncated = true;
        self.buf
            .push_str(prefix_at_char_boundary(output, remaining));
    }

    fn append_char(&mut self, ch: char) {
        self.total_bytes_seen += ch.len_utf8();
        if self.buf.len() >= self.max_bytes {
            self.truncated = true;
            return;
        }

        if self.buf.len() + ch.len_utf8() <= self.max_bytes {
            self.buf.push(ch);
        } else {
            self.truncated = true;
        }
    }

    pub(crate) fn into_string(self) -> String {
        if !self.truncated {
            return self.buf;
        }

        let captured_bytes = self.buf.len();
        let total_bytes_seen = self.total_bytes_seen;
        let buf = self.buf;
        format!(
            "{}...\n[print output truncated, captured {} of {} bytes]",
            buf, captured_bytes, total_bytes_seen,
        )
    }
}

impl PrintWriterCallback for BoundedOutputCollector {
    fn stdout_write(&mut self, output: Cow<'_, str>) -> Result<(), MontyException> {
        self.append_str(&output);
        Ok(())
    }

    fn stdout_push(&mut self, end: char) -> Result<(), MontyException> {
        self.append_char(end);
        Ok(())
    }
}

pub(crate) fn truncate_output(output: &str) -> String {
    if output.len() > MAX_TOOL_RESULT_BYTES {
        let shown = prefix_at_char_boundary(output, MAX_TOOL_RESULT_BYTES);
        format!(
            "{}...\n[truncated, showing {} of {} bytes]",
            shown,
            shown.len(),
            output.len()
        )
    } else {
        output.to_string()
    }
}

pub(crate) fn prefix_at_char_boundary(input: &str, max_bytes: usize) -> &str {
    if input.len() <= max_bytes {
        return input;
    }
    if max_bytes == 0 {
        return "";
    }

    let mut end = 0;
    for (idx, ch) in input.char_indices() {
        let next = idx + ch.len_utf8();
        if next > max_bytes {
            break;
        }
        end = next;
    }
    &input[..end]
}
