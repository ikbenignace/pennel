def pennel_post_install(installer)
  native_targets = installer.pods_project.targets.group_by(&:name)
  installer.pod_targets.each do |pod|
    targets = native_targets[pod.label] || []
    next if targets.empty?
    next if ['PennelMobile', 'PennelMobileKit'].include?(pod.pod_name)
    declared_versions = pod.specs.flat_map do |spec|
      Array(spec.attributes_hash['swift_versions'] || spec.attributes_hash['swift_version'])
    end.compact
    next unless declared_versions.empty?

    # Unspecified SDK pods otherwise inherit Swift 6 from Pennel instead of Expo's stock Swift 5 consumer.
    targets.each do |target|
      target.build_configurations.each do |configuration|
        configuration.build_settings['SWIFT_VERSION'] = '5.0'
      end
    end
  end
end
