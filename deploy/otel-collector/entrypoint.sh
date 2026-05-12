#!/bin/sh
set -eu

if [ -n "${DD_API_KEY:-}" ]; then
  echo "DD_API_KEY detected - exporting to Datadog"
  exec /otelcol-contrib \
    --feature-gates=datadog.EnableOperationAndResourceNameV2 \
    --config /etc/otelcol-contrib/otel-datadog.yaml
fi

echo "No DD_API_KEY - running in debug mode (traces logged to stdout)"
echo "To enable Datadog: add DD_API_KEY to this service in Railway"
exec /otelcol-contrib --config /etc/otelcol-contrib/otel-debug.yaml
