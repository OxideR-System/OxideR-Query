# OxideR-Query task runner.
#
# Run from a POSIX shell. On Windows that means Git Bash, not cmd or PowerShell.
#
#   make            list every target
#   make check      the gate CI runs, before you push
#   make release VERSION=0.1.1
#
# `release` is the only target that touches anything outside the working tree.
# It refuses to start unless the tree is clean, the branch is the release
# branch, the tag is free and the full gate passes, so a failed release leaves
# nothing half-done.

SHELL := /bin/bash
.SHELLFLAGS := -eu -o pipefail -c
.DEFAULT_GOAL := help

CARGO ?= cargo
GH ?= gh

# Every cargo invocation here spans the workspace with every feature on. A
# target that checked less than CI would report success CI then contradicts.
WORKSPACE := --workspace --all-features
RELEASE_BRANCH ?= main
REMOTE ?= origin

# The version the manifests currently declare, used to rewrite them in place.
CURRENT_VERSION = $(shell grep -m1 '^version = ' Cargo.toml | cut -d'"' -f2)

.PHONY: help fmt fmt-check lint test test-doc check bench audit package publish-dry \
        docs-serve docs-build clean version release pg-up pg-down test-pg

help: ## List the targets
	@echo "OxideR-Query - available targets"
	@echo
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) \
		| awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-14s\033[0m %s\n", $$1, $$2}'
	@echo
	@echo "  current version: $(CURRENT_VERSION)"

fmt: ## Format every crate
	$(CARGO) fmt --all

fmt-check: ## Fail if anything is unformatted
	$(CARGO) fmt --all --check

lint: ## Clippy over every target, warnings are errors
	$(CARGO) clippy $(WORKSPACE) --all-targets -- -D warnings

test: ## Unit, integration and compile-fail tests
	$(CARGO) test $(WORKSPACE) --all-targets

# `--all-targets` silently skips doc tests, and every compile-time guarantee
# this library advertises is pinned by a `compile_fail` doc test, so the two
# runs are not interchangeable.
test-doc: ## Doc tests, including the compile_fail guarantees
	$(CARGO) test $(WORKSPACE) --doc

check: fmt-check lint test test-doc ## Everything CI runs
	@echo "check: clean"

# The Postgres suite skips itself unless OXIDER_POSTGRES_URL is set, so these
# three targets are the whole local setup: bring a server up, run against it,
# throw it away.
PG_CONTAINER ?= oxider-pg
PG_PORT ?= 54329
PG_URL ?= postgres://postgres:oxider@localhost:$(PG_PORT)/oxider

pg-up: ## Start a throwaway PostgreSQL for the end-to-end suite
	@docker run -d --name $(PG_CONTAINER) \
		-e POSTGRES_PASSWORD=oxider -e POSTGRES_DB=oxider \
		-p $(PG_PORT):5432 postgres:16 >/dev/null
	@printf 'waiting for postgres'
	@for i in $$(seq 1 30); do \
		if docker exec $(PG_CONTAINER) pg_isready -U postgres -d oxider >/dev/null 2>&1; then \
			echo " ready on $(PG_URL)"; exit 0; \
		fi; \
		printf '.'; sleep 1; \
	done; \
	echo " gave up"; exit 1

pg-down: ## Remove the throwaway PostgreSQL
	@docker rm -f $(PG_CONTAINER) >/dev/null 2>&1 || true
	@echo "removed $(PG_CONTAINER)"

test-pg: ## Run the PostgreSQL end-to-end suite against a running server
	OXIDER_POSTGRES_URL=$(PG_URL) $(CARGO) test -p oxider-query-exec \
		--features postgres --test postgres_end_to_end -- --test-threads=1

bench: ## Render-throughput microbench (criterion)
	$(CARGO) bench -p oxider-query

audit: ## Check dependencies for security advisories
	@command -v cargo-audit >/dev/null 2>&1 || { \
		echo "cargo-audit is not installed: cargo install cargo-audit --locked"; \
		exit 1; \
	}
	$(CARGO) audit

package: ## List what each crate would ship to crates.io
	@for crate in oxider-query-macros oxider-query-core oxider-query-exec oxider-query-codegen oxider-query; do \
		echo "=== $$crate ==="; \
		$(CARGO) package --list -p $$crate --allow-dirty | head -40; \
	done

# Publishing has to happen in dependency order, and each crate can only see the
# ones already on crates.io, so the dry run is per crate rather than workspace
# wide.
publish-dry: ## Dry-run a crates.io publish, in dependency order
	@for crate in oxider-query-macros oxider-query-core oxider-query-exec oxider-query-codegen oxider-query; do \
		echo "=== $$crate ==="; \
		$(CARGO) publish --dry-run -p $$crate --allow-dirty; \
	done

docs-serve: ## Docusaurus dev server for the guide
	cd website && npm install && npm start

docs-build: ## Build the guide as a static site
	cd website && npm install && npm run build

clean: ## Remove build artifacts
	$(CARGO) clean

version: ## Print the version the manifests declare
	@echo $(CURRENT_VERSION)

# --- release ------------------------------------------------------------
#
# NOTES=path/to/notes.md supplies release notes; without it GitHub generates
# them from the commits since the previous tag.
NOTES ?=

release: ## Bump, verify, tag and publish a GitHub release. VERSION=x.y.z required
	@test -n "$(VERSION)" || { echo "usage: make release VERSION=x.y.z"; exit 1; }
	@[[ "$(VERSION)" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$$ ]] \
		|| { echo "VERSION must be semver, got '$(VERSION)'"; exit 1; }
	@command -v $(GH) >/dev/null 2>&1 || { echo "gh is not installed"; exit 1; }
	@$(GH) auth status >/dev/null 2>&1 || { echo "gh is not authenticated: gh auth login"; exit 1; }
	@test -z "$$(git status --porcelain --untracked-files=no)" \
		|| { echo "working tree is dirty; commit or stash first"; exit 1; }
	@test "$$(git branch --show-current)" = "$(RELEASE_BRANCH)" \
		|| { echo "not on $(RELEASE_BRANCH)"; exit 1; }
	@! git rev-parse -q --verify "refs/tags/v$(VERSION)" >/dev/null \
		|| { echo "tag v$(VERSION) already exists"; exit 1; }
	@git fetch --quiet $(REMOTE) $(RELEASE_BRANCH)
	@# Being ahead of the remote is the normal state before a release, since the
	@# release pushes. Being behind is not: it would tag a commit that does not
	@# contain what is already published.
	@test "$$(git rev-list --count HEAD..$(REMOTE)/$(RELEASE_BRANCH))" = "0" \
		|| { echo "$(REMOTE)/$(RELEASE_BRANCH) has commits you do not: pull first"; exit 1; }
	@test -z "$(NOTES)" || test -f "$(NOTES)" || { echo "no such notes file: $(NOTES)"; exit 1; }
	@echo "releasing v$(VERSION) (from $(CURRENT_VERSION))"
	@if [ "$(CURRENT_VERSION)" != "$(VERSION)" ]; then \
		sed -i 's/version = "$(CURRENT_VERSION)"/version = "$(VERSION)"/g' Cargo.toml; \
		$(CARGO) check $(WORKSPACE) --quiet; \
		git add Cargo.toml Cargo.lock; \
		git commit -q -m "release: v$(VERSION)"; \
		echo "bumped $(CURRENT_VERSION) -> $(VERSION)"; \
	else \
		echo "manifests already declare $(VERSION), no bump commit"; \
	fi
	@$(MAKE) --no-print-directory check
	@git tag -a "v$(VERSION)" -m "v$(VERSION)"
	@git push --quiet $(REMOTE) $(RELEASE_BRANCH)
	@git push --quiet $(REMOTE) "v$(VERSION)"
	@if [ -n "$(NOTES)" ]; then \
		$(GH) release create "v$(VERSION)" --title "v$(VERSION)" --notes-file "$(NOTES)"; \
	else \
		$(GH) release create "v$(VERSION)" --title "v$(VERSION)" --generate-notes; \
	fi
	@echo "released v$(VERSION)"
