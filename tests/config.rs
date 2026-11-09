use ministat::config::Config;

fn load(external_mount: &str) -> std::io::Result<Config> {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.json");
    let config = serde_json::json!({
        "external_mount": external_mount,
        "homekit_data": "/tmp/homekit",
        "jellyfin_port": 8096,
        "runner_roots": [],
    });
    std::fs::write(&path, config.to_string()).unwrap();
    Config::load(&path)
}

#[test]
fn a_trailing_separator_on_the_external_mount_is_accepted() {
    assert!(load("/Volumes/Media/").is_ok());
}

#[test]
fn parent_components_in_the_external_mount_are_rejected() {
    assert!(load("/Volumes/Other/../Media").is_err());
    assert!(load("Volumes/Media").is_err());
}

fn runner(parent: &std::path::Path, name: &str, registered: bool) -> std::path::PathBuf {
    let root = parent.join(name);
    std::fs::create_dir_all(root.join("bin")).unwrap();
    std::fs::write(root.join("bin/Runner.Listener"), "").unwrap();
    if registered {
        std::fs::write(root.join(".runner"), "{}").unwrap();
    }
    root
}

#[test]
fn registered_runners_are_discovered_in_name_order() {
    let parent = tempfile::tempdir().unwrap();
    let b = runner(parent.path(), "runner-b", true);
    let a = runner(parent.path(), "runner-a", true);
    runner(parent.path(), "deregistered", false);
    std::fs::create_dir(parent.path().join("empty")).unwrap();
    std::fs::write(parent.path().join("notes.txt"), "").unwrap();
    std::os::unix::fs::symlink(&a, parent.path().join("alias")).unwrap();
    assert_eq!(
        ministat::config::discover_runners(parent.path()),
        vec![a, b]
    );
}

#[test]
fn a_missing_runner_directory_discovers_nothing() {
    let parent = tempfile::tempdir().unwrap();
    assert!(ministat::config::discover_runners(&parent.path().join("absent")).is_empty());
}

#[test]
fn runner_roots_are_discovered_unless_configured() {
    let home = tempfile::tempdir().unwrap();
    let runners = home.path().join("actions-runners");
    let found = runner(&runners, "found", true);
    let mut config = Config::for_home(home.path());
    assert_eq!(config.runner_roots(), vec![found]);
    config.runner_roots = Some(vec!["/explicit/runner".into()]);
    assert_eq!(
        config.runner_roots(),
        vec![std::path::PathBuf::from("/explicit/runner")]
    );
}

#[test]
fn a_config_file_may_omit_runner_roots() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.json");
    let config = serde_json::json!({
        "external_mount": "/Volumes/Media",
        "homekit_data": "/tmp/homekit",
        "jellyfin_port": 8096,
    });
    std::fs::write(&path, config.to_string()).unwrap();
    assert!(Config::load(&path).unwrap().runner_roots.is_none());
}

#[test]
fn hermes_and_tailscale_settings_have_defaults_and_must_be_absolute() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.json");
    let mut value = serde_json::json!({
        "external_mount": "/Volumes/Media",
        "homekit_data": "/tmp/homekit",
        "jellyfin_port": 8096,
    });
    std::fs::write(&path, value.to_string()).unwrap();
    let config = Config::load(&path).unwrap();
    assert_eq!(config.hermes_port, 9120);
    assert_eq!(
        config.tailscale_cli,
        std::path::PathBuf::from("/Applications/Tailscale.app/Contents/MacOS/Tailscale")
    );
    value["tailscale_cli"] = "tailscale".into();
    std::fs::write(&path, value.to_string()).unwrap();
    assert!(Config::load(&path).is_err());
}
