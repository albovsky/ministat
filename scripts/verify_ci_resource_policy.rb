#!/usr/bin/env ruby
# frozen_string_literal: true
require 'yaml'
require 'pathname'

module CIResourcePolicy
  POOLS = [%w[self-hosted macOS ARM64 ministat-ci]].freeze
  # Changing these pins requires reviewing the action's artifact/cache behavior.
  ACTIONS = ['actions/checkout@11d5960a326750d5838078e36cf38b85af677262'].freeze
  module_function
  def steps!(steps, root, visited)
    Array(steps).each do |step|
      action = step['uses']
      next unless action
      if action.start_with?('./')
        base = File.expand_path(action,root)
        raise 'Local action escapes repository' unless base.start_with?(root+'/')
        path = %w[action.yml action.yaml].map { |file| File.join(base,file) }.find { |file| File.file?(file) }
        raise 'Local action missing or cyclic' unless path && !visited.include?(path)
        raise 'Local action symlink escapes repository' unless File.realpath(path).start_with?(File.realpath(root)+'/')
        doc = YAML.safe_load(File.read(path),aliases:false)
        raise 'Only inspected composite actions are allowed' unless doc.dig('runs','using')=='composite'
        steps!(doc.dig('runs','steps'),root,visited+[path])
      else
        raise "Unapproved action (uploads/caches prohibited): #{action}" unless ACTIONS.include?(action)
        if action.start_with?('actions/setup-node@')
          inputs = step.fetch('with',{})
          raise 'setup-node caching must be explicitly disabled' unless inputs['package-manager-cache']==false && !inputs.key?('cache')
        end
      end
    end
  end
  def check(root)
    root = File.expand_path(root)
    paths = Dir.glob(File.join(root,'.github/workflows/*.{yml,yaml}'))
    raise 'No workflows found' if paths.empty?
    paths.each do |path|
      document = YAML.safe_load(File.read(path),aliases:false)
      document.fetch('jobs').each do |name,job|
        raise "#{path}: #{name} must use an explicit approved Mini pool" unless POOLS.include?(job['runs-on']) && !job.key?('uses')
        steps!(job['steps'],root,[])
      end
    end
  end
end
if $PROGRAM_NAME == __FILE__
  begin
    CIResourcePolicy.check(ARGV.fetch(0,File.expand_path('..',__dir__)))
    puts 'CI resource policy passed: Mini execution and local evidence only.'
  rescue StandardError => error
    warn "CI resource policy: #{error.message}"
    exit 1
  end
end
