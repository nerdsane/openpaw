SHELL := /bin/bash

.PHONY: setup dev build wasm dashboard check docker clean deploy deploy-observability

setup:
	./scripts/setup-dev.sh

dev:
	cargo run -p temperpaw

build:
	cd dashboard && npm run build
	cargo build -p temperpaw --release

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
