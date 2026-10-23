.PHONY: core test app run

core:
	scripts/build-core.sh

test: core
	cargo test --manifest-path core/Cargo.toml
	swift test --package-path apps/macos

app: core
	scripts/bundle-app.sh

run: app
	open build/Musli.app
