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
ARG KATAGAMI_REF=main
RUN apt-get update && apt-get install -y --no-install-recommends git ca-certificates && rm -rf /var/lib/apt/lists/* \
    && rm -f os-apps/katagami-curation os-apps/katagami-commons \
    && git clone --depth 1 --branch "${KATAGAMI_REF}" https://github.com/arni-labs/katagami.git /tmp/katagami \
    && cp -a /tmp/katagami/katagami-curation os-apps/katagami-curation \
    && cp -a /tmp/katagami/katagami-commons  os-apps/katagami-commons  \
    && rm -rf /tmp/katagami
COPY scripts ./scripts
COPY docs ./docs
COPY railway.toml README.md AGENTS.md CLAUDE.md INSTRUCTIONS.md ./
COPY --from=dashboard-build /app/dashboard/build ./dashboard/build
ENV CARGO_BUILD_JOBS=2
ENV BUILD_VERSION=${BUILD_VERSION}
ENV BUILD_SHA=${BUILD_SHA}
RUN cargo build -p openpaw --release --bin openpaw-server
# Build WASM modules for os-apps (requires wasm32 targets)
RUN rustup target add wasm32-unknown-unknown wasm32-wasip1
RUN cd os-apps/paw-agent/wasm && bash build.sh \
    && cd /app/os-apps/paw-channels/wasm && bash build.sh \
    && cd /app/os-apps/paw-fs/wasm/blob_adapter && bash build.sh \
    && cd /app/os-apps/paw-fs/wasm/workspace_fs && bash build.sh \
    && cd /app/os-apps/paw-research/wasm && bash build.sh \
    && cd /app/os-apps/katagami-curation/wasm && bash build.sh

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates libz3-4 git && rm -rf /var/lib/apt/lists/*
ARG BUILD_VERSION=dev
ARG BUILD_SHA=unknown
WORKDIR /app
COPY --from=rust-build /app/target/release/openpaw-server ./openpaw
COPY --from=rust-build /app/dashboard/build ./dashboard/build
COPY --from=rust-build /app/os-apps ./os-apps
ENV BUILD_VERSION=${BUILD_VERSION}
ENV BUILD_SHA=${BUILD_SHA}
EXPOSE 3467
CMD ["./openpaw"]
