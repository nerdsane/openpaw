SHELL := /bin/bash

.PHONY: setup dev lapdog lapdog-run lapdog-env lapdog-doctor build wasm dashboard dashboard-build check docker clean deploy deploy-observability

setup:
	./scripts/setup-dev.sh

dev:
	cargo run -p temperpaw

lapdog:
	./scripts/run-lapdog-local.sh --lapdog-only

lapdog-run: dashboard/build/index.html
	./scripts/run-lapdog-local.sh

lapdog-env:
	./scripts/run-lapdog-local.sh --print-env

lapdog-doctor:
	./scripts/run-lapdog-local.sh doctor

build:
	cd dashboard && npm run build
	cargo build -p temperpaw --release

dashboard-build:
	cd dashboard && npm run build

dashboard/build/index.html:
	cd dashboard && npm run build

wasm:
	cargo build --workspace --target wasm32-unknown-unknown

dashboard:
	cd dashboard && npm run dev

check:
	cargo test -p temperpaw --quiet
	cd dashboard && npm run build

docker:
	docker build -t temperpaw:dev .

clean:
	cargo clean
	rm -rf dashboard/build dashboard/.svelte-kit

deploy:
	cargo run -p temperpaw -- deploy

deploy-observability:
	python3 scripts/deploy_dashboard.py
	python3 scripts/deploy_monitors.py
