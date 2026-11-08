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
