# frozen_string_literal: true
require 'minitest/autorun'
require 'tmpdir'
require 'fileutils'
require 'open3'
require 'yaml'
class CIResourcePolicyTest < Minitest::Test
  SCRIPT = File.expand_path('verify_ci_resource_policy.rb',__dir__)
  def check(job, actions = {})
    Dir.mktmpdir do |root|
      FileUtils.mkdir_p(File.join(root,'.github/workflows'))
      File.write(File.join(root,'.github/workflows/test.yml'), YAML.dump('jobs'=>{'test'=>job}))
      actions.each do |path,document|
        FileUtils.mkdir_p(File.dirname(File.join(root,path)))
        File.write(File.join(root,path),YAML.dump(document))
      end
      Open3.capture3('ruby',SCRIPT,root)
    end
  end
  def local_job(steps = [])
    {'runs-on'=>%w[self-hosted macOS ARM64 ministat-ci], 'steps'=>steps}
  end
  def test_hosted_dynamic_and_reusable_jobs_are_rejected
    ['ubuntu-latest', {'group'=>'hosted'}, '${{ matrix.runner }}', ['self-hosted','ubuntu-latest']].each do |runner|
      refute check('runs-on'=>runner,'steps'=>[]).last.success?
    end
    refute check('uses'=>'owner/repo/.github/workflows/build.yml@main').last.success?
    assert check(local_job).last.success?
  end
  def test_artifact_and_cache_variants_and_implicit_setup_cache_are_rejected
    %w[actions/upload-artifact actions/download-artifact actions/cache actions/cache/save actions/cache/restore].each do |action|
      refute check(local_job([{'uses'=>"#{action}@#{'a'*40}"}])).last.success?
    end
    refute check(local_job([{'uses'=>"actions/setup-node@820762786026740c76f36085b0efc47a31fe5020"}])).last.success?
    assert check(local_job([{'uses'=>"actions/checkout@11d5960a326750d5838078e36cf38b85af677262"}])).last.success?
  end
  def test_nested_composite_upload_and_unknown_action_are_rejected
    job = local_job([{'uses'=>'./.github/actions/nested'}])
    refute check(job,{'.github/actions/nested/action.yml'=>{'runs'=>{'using'=>'composite','steps'=>[{'uses'=>"actions/upload-artifact@#{'a'*40}"}]}}}).last.success?
    refute check(local_job([{'uses'=>"unknown/build@#{'a'*40}"}])).last.success?
  end
end
