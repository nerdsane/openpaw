SHELL := /bin/bash

.PHONY: setup dev build wasm dashboard check docker clean deploy deploy-observability

setup:
	./scripts/setup-dev.sh

dev:
	cargo run -p openpaw

build:
	cd dashboard && npm run build
	cargo build -p openpaw --release

wasm:
	cargo build --workspace --target wasm32-unknown-unknown

dashboard:
	cd dashboard && npm run dev

check:
	cargo test -p openpaw --quiet
	cd dashboard && npm run build

docker:
	docker build -t openpaw:dev .

clean:
	cargo clean
	rm -rf dashboard/build dashboard/.svelte-kit

deploy:
	cargo run -p openpaw -- deploy

deploy-observability:
	python3 scripts/deploy_dashboard.py
	python3 scripts/deploy_monitors.py
