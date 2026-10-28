require 'json'

package = JSON.parse(File.read(File.join(__dir__, 'package.json')))

Pod::Spec.new do |s|
  s.name = 'PennelMobile'
  s.version = package['version']
  s.summary = 'Pennel native runtime and shared Rust core for Expo'
  s.homepage = 'https://github.com/pennel-app/pennel'
  s.license = { :type => 'Apache-2.0' }
  s.author = 'Pennel'
  s.source = { :git => 'https://github.com/pennel-app/pennel.git', :tag => s.version.to_s }
  s.platform = :ios, '26.0'
  s.swift_version = '6.0'
  s.static_framework = true
  s.dependency 'ExpoModulesCore'
  s.dependency 'PennelMobileKit'
  s.source_files = [
    'ios/**/*.swift',
    'generated/ios/Bindings/*.swift',
    'generated/ios/Services/**/*.swift'
  ]
  s.vendored_frameworks = 'generated/ios/PennelCoreFFI.xcframework'
  s.frameworks = 'AVFoundation', 'Speech', 'FoundationModels', 'ActivityKit', 'BackgroundTasks', 'AppIntents', 'Security', 'SystemConfiguration'
  s.libraries = 'c++', 'resolv'
  s.pod_target_xcconfig = { 'DEFINES_MODULE' => 'YES', 'SWIFT_COMPILATION_MODE' => 'wholemodule' }
end
