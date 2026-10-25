.PHONY: core test app app-preview run desktop desktop-preview desktop-test packaging-test theme theme-check

core: theme
	scripts/with-toolchain.sh scripts/build-core.sh

test: core
	scripts/with-toolchain.sh cargo test --manifest-path core/Cargo.toml
	swift test --package-path apps/macos

app: core
	scripts/bundle-app.sh

app-preview: core
	scripts/bundle-app.sh --preview

run: app
	open build/Pennel.app

desktop: theme
	cd apps/desktop && npm ci && ../../scripts/with-toolchain.sh npx tauri build

desktop-preview: theme
	scripts/build-desktop-preview.sh

desktop-test: theme
	cd apps/desktop && npm ci && npm run build
	scripts/with-toolchain.sh cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml

packaging-test:
	python3 -B -m unittest discover -s scripts/tests -p 'test_desktop_*.py'

theme:
	python3 scripts/generate-theme.py

theme-check:
	python3 scripts/generate-theme.py --check
