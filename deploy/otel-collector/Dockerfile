FROM otel/opentelemetry-collector-contrib:latest AS collector

FROM alpine:3.20

RUN apk add --no-cache ca-certificates

COPY --from=collector /otelcol-contrib /otelcol-contrib

COPY otel-datadog.yaml /etc/otelcol-contrib/otel-datadog.yaml
COPY otel-debug.yaml /etc/otelcol-contrib/otel-debug.yaml
COPY --chmod=755 entrypoint.sh /entrypoint.sh

ENTRYPOINT ["/entrypoint.sh"]
