.PHONY: core test app run desktop desktop-preview desktop-test packaging-test

core:
	scripts/with-toolchain.sh scripts/build-core.sh

test: core
	scripts/with-toolchain.sh cargo test --manifest-path core/Cargo.toml
	swift test --package-path apps/macos

app: core
	scripts/bundle-app.sh

run: app
	open build/Pennel.app

desktop:
	cd apps/desktop && npm ci && ../../scripts/with-toolchain.sh npx tauri build

desktop-preview:
	cd apps/desktop && npm ci && ../../scripts/with-toolchain.sh npx tauri build --config src-tauri/tauri.preview.conf.json

desktop-test:
	cd apps/desktop && npm ci && npm run build
	scripts/with-toolchain.sh cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml

packaging-test:
	python3 -B -m unittest discover -s scripts/tests -p 'test_desktop_*.py'
