.PHONY: core test app app-preview run desktop desktop-preview desktop-test packaging-test theme theme-check ios-core ios ios-test mobile-core mobile-prebuild mobile-test mobile-ios mobile-native-test

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

IOS_DESTINATION ?= platform=iOS Simulator,name=iPhone 17

ios-core:
	scripts/with-toolchain.sh scripts/build-core-ios.sh

IOS_DERIVED_DATA ?= apps/ios/build/DerivedData

ios: ios-core
	xcodebuild build -project apps/ios/Pennel.xcodeproj -scheme Pennel -configuration Release \
		-destination 'generic/platform=iOS' -derivedDataPath $(IOS_DERIVED_DATA) CODE_SIGNING_ALLOWED=NO
	@app=$(IOS_DERIVED_DATA)/Build/Products/Release-iphoneos/Pennel.app; \
		echo "unsigned Release Pennel.app: $$(find $$app -type f -exec stat -f %z {} + | awk '{s+=$$1} END {print s}') bytes"

ios-test: ios-core
	swift test --package-path apps/ios/PennelMobileKit
	xcodebuild test -project apps/ios/Pennel.xcodeproj -scheme Pennel -destination '$(IOS_DESTINATION)' \
		-derivedDataPath $(IOS_DERIVED_DATA)

mobile-core:
	cd apps/mobile && npm run native:prepare

mobile-prebuild: mobile-core
	cd apps/mobile && npm run test:prebuild
	cd apps/mobile && npm run prebuild:clean
	cd apps/mobile/ios && pod install
	cd apps/mobile && npm run test:pods

mobile-test:
	cd apps/mobile && npm run typecheck && npm run test:plugins && npm run test:widgets && npm run test:ui
	swift test --package-path apps/ios/PennelMobileKit

mobile-ios: mobile-prebuild
	cd apps/mobile && npm run build:ios

mobile-native-test:
	bash scripts/ci-xcodebuild.sh test -workspace apps/mobile/ios/Pennel.xcworkspace -scheme Pennel -configuration Debug \
		-destination '$(IOS_DESTINATION)' -derivedDataPath apps/mobile/build/TestDerivedData \
		-only-testing:PennelRuntimeTests -only-testing:PennelKeyboardTests \
		CODE_SIGNING_ALLOWED=YES CODE_SIGN_IDENTITY=- CODE_SIGN_STYLE=Manual DEVELOPMENT_TEAM=
