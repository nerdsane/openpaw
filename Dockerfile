FROM node:22-bookworm AS dashboard-build
WORKDIR /app/dashboard
COPY dashboard/package*.json ./
RUN npm install
COPY dashboard ./
RUN npm run build

FROM rust:1.94-bookworm AS rust-build
RUN apt-get update && apt-get install -y libclang-dev cmake libz3-dev && rm -rf /var/lib/apt/lists/*
ARG BUILD_VERSION=dev
ARG BUILD_SHA=unknown
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY os-apps ./os-apps
# `os-apps/katagami-curation` and `os-apps/katagami-commons` are symlinks
# into a local developer's checkout of github.com/arni-labs/katagami. The
# symlinks work on a developer's Mac but resolve to a non-existent path
# inside the Docker build, which is why `Startup OS app surface resolved
# from manifests` reported only ["paw-agent","paw-channels","paw-fs",
# "paw-research"] and the Katagami data layer was missing every deploy.
#
# Replace the symlinks with real content pulled from the upstream repo
# on every image build so the catalog discovers them. `--depth 1` keeps
# the download tiny. Pin via KATAGAMI_REF arg when a reproducible image
# is needed; defaults to main.
ARG KATAGAMI_REF=master
ARG TEMPER_OBSERVABILITY_REV=eafba6abf5adf534a4b2578887252f196980b7a6
RUN apt-get update && apt-get install -y --no-install-recommends git ca-certificates && rm -rf /var/lib/apt/lists/* \
    && rm -rf os-apps/katagami-curation os-apps/katagami-commons \
    && git clone --depth 1 --branch "${KATAGAMI_REF}" https://github.com/arni-labs/katagami.git /tmp/katagami \
    && cp -a /tmp/katagami/katagami-curation os-apps/katagami-curation \
    && cp -a /tmp/katagami/katagami-commons  os-apps/katagami-commons  \
    && find os-apps/katagami-curation/wasm -name Cargo.toml -exec sed -i "s|temper-wasm-sdk = { git = \"https://github.com/nerdsane/temper.git\", branch = \"main\" }|temper-wasm-sdk = { git = \"https://github.com/nerdsane/temper.git\", rev = \"${TEMPER_OBSERVABILITY_REV}\" }|g" {} + \
    && find os-apps/katagami-curation/wasm -name Cargo.lock -delete \
    && rm -rf /tmp/katagami
COPY scripts ./scripts
COPY docs ./docs
COPY railway.toml README.md AGENTS.md CLAUDE.md INSTRUCTIONS.md ./
COPY --from=dashboard-build /app/dashboard/build ./dashboard/build
ENV CARGO_BUILD_JOBS=2
ENV BUILD_VERSION=${BUILD_VERSION}
ENV BUILD_SHA=${BUILD_SHA}
RUN cargo build -p temperpaw --release --bin temperpaw-server
# Build WASM modules for os-apps (requires wasm32 targets)
RUN rustup target add wasm32-unknown-unknown wasm32-wasip1
RUN cd os-apps/paw-agent/wasm && bash build.sh \
    && cd /app/os-apps/paw-channels/wasm && bash build.sh \
    && cd /app/os-apps/paw-fs/wasm/blob_adapter && bash build.sh \
    && cd /app/os-apps/paw-fs/wasm/workspace_fs && bash build.sh \
    && cd /app/os-apps/paw-ingest/wasm && bash build.sh \
    && cd /app/os-apps/paw-managed-agents/wasm && bash build.sh \
    && cd /app/os-apps/paw-skills/wasm && bash build.sh \
    && cd /app/os-apps/paw-research/wasm && bash build.sh \
    && cd /app/os-apps/paw-patrol/wasm && bash build.sh \
    && cd /app/os-apps/katagami-curation/wasm && bash build.sh
RUN find os-apps -type d -name target -prune -exec rm -rf {} +

FROM debian:bookworm-slim
ARG TARGETARCH
ARG DDPROF_VERSION=0.26.0
RUN apt-get update \
    && apt-get install -y ca-certificates curl libz3-4 git xz-utils \
    && rm -rf /var/lib/apt/lists/* \
    && case "${TARGETARCH:-amd64}" in \
        amd64|arm64) ddprof_arch="${TARGETARCH:-amd64}" ;; \
        *) echo "unsupported ddprof architecture: ${TARGETARCH}" >&2; exit 1 ;; \
    esac \
    && curl -fsSL "https://github.com/DataDog/ddprof/releases/download/v${DDPROF_VERSION}/ddprof-${DDPROF_VERSION}-${ddprof_arch}-linux.tar.xz" -o /tmp/ddprof.tar.xz \
    && tar -xJf /tmp/ddprof.tar.xz -C /usr/local/bin --strip-components=2 ddprof/bin/ddprof \
    && chmod +x /usr/local/bin/ddprof \
    && rm -f /tmp/ddprof.tar.xz
ARG BUILD_VERSION=dev
ARG BUILD_SHA=unknown
WORKDIR /app
COPY --from=rust-build /app/target/release/temperpaw-server ./temperpaw
COPY --from=rust-build /app/dashboard/build ./dashboard/build
COPY --from=rust-build /app/os-apps ./os-apps
COPY scripts/temperpaw-entrypoint.sh ./scripts/temperpaw-entrypoint.sh
COPY scripts/datadog_railway_capability_check.sh ./scripts/datadog_railway_capability_check.sh
RUN chmod +x ./scripts/temperpaw-entrypoint.sh
RUN chmod +x ./scripts/datadog_railway_capability_check.sh
ENV BUILD_VERSION=${BUILD_VERSION}
ENV BUILD_SHA=${BUILD_SHA}
EXPOSE 3467
ENTRYPOINT ["./scripts/temperpaw-entrypoint.sh"]
