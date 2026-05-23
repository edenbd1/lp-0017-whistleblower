# LP-0017 dev wrappers.
#
# Targets are thin Bash wrappers over cargo + scripts/. The point is
# to give every contributor the same one-line entry points (matching
# the README quickstart) without locking us into Make-specific syntax.

.PHONY: help build test fmt clippy ci-local demo deploy stack stack-down idl bench clean

help: ## Show this help
	@awk 'BEGIN {FS = ":.*##"; printf "Available targets:\n\n"} /^[a-zA-Z_-]+:.*##/ { printf "  \033[36m%-15s\033[0m %s\n", $$1, $$2 }' $(MAKEFILE_LIST)

build: ## Build the host workspace (excludes the SPEL guest)
	cargo build --workspace --exclude whistleblower-registry-guest

test: ## Run the host workspace tests
	cargo test --workspace --exclude whistleblower-registry-guest

fmt: ## Reformat every file in place
	cargo fmt --all

clippy: ## Lint with the same flags CI uses
	cargo clippy --workspace --all-targets -- -D warnings

ci-local: ## Mirror the fast CI tier locally before pushing
	bash scripts/ci-local.sh

stack: ## Bring up nwaku + storage via docker compose
	docker compose -f infra/docker-compose.yml up -d
	@echo "nwaku:    http://127.0.0.1:8645"
	@echo "storage:  http://127.0.0.1:8080"

stack-down: ## Stop the local nwaku + storage stack
	docker compose -f infra/docker-compose.yml down

idl: ## Regenerate the SPEL IDL from the guest source
	spel generate-idl methods/guest/src/bin/whistleblower_registry.rs > idl/whistleblower_registry.idl.json
	@jq '.name, (.instructions | length)' idl/whistleblower_registry.idl.json

deploy: ## Build the guest + deploy + print the program_id
	bash scripts/deploy.sh

demo: ## Full end-to-end demo with RISC0_DEV_MODE=0
	bash scripts/demo.sh

bench: ## Run the e2e anchor round-trip behind the live-lez feature
	cargo test -p batch-anchor --features live-lez --test e2e_anchor -- --include-ignored --nocapture

clean: ## cargo clean + drop generated artefacts
	cargo clean
	rm -rf .demo-state .basecamp-data nwaku-store storage-data
