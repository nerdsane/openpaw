FROM node:22-bookworm AS dashboard-build
WORKDIR /app/dashboard
COPY dashboard/package*.json ./
RUN npm install
COPY dashboard ./
RUN npm run build

FROM rust:1.94-bookworm AS rust-build
RUN apt-get update && apt-get install -y libclang-dev cmake libz3-dev && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY os-apps ./os-apps
COPY scripts ./scripts
COPY docs ./docs
COPY railway.toml README.md AGENTS.md CLAUDE.md INSTRUCTIONS.md ./
COPY --from=dashboard-build /app/dashboard/build ./dashboard/build
ENV CARGO_BUILD_JOBS=2
RUN cargo build -p openpaw --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=rust-build /app/target/release/openpaw ./openpaw
COPY --from=rust-build /app/dashboard/build ./dashboard/build
COPY os-apps ./os-apps
EXPOSE 3467
CMD ["./openpaw"]
