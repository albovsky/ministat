use ministat::{
    metrics::{Sampler, disks},
    model::{Health, now_unix},
};

#[test]
fn an_existing_directory_is_not_a_mounted_external_drive() {
    let directory = tempfile::tempdir().unwrap();
    let rows = disks(directory.path());
    assert!(rows[0].mounted);
    assert!(!rows[1].mounted);
    assert_eq!(rows[1].used_percent, None);
}

#[test]
fn cpu_warms_up_before_reporting_a_percentage() {
    let mut sampler = Sampler::default();
    let storage = disks(std::path::Path::new("/Volumes/MinistatTestMissing"));
    let first = sampler.sample(storage.clone(), now_unix(), Health::default());
    assert_eq!(first.cpu_percent, None);
    std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
    let second = sampler.sample(storage, now_unix(), Health::default());
    let cpu = second.cpu_percent.unwrap();
    assert!(cpu.is_finite() && (0.0..=100.0).contains(&cpu));
    assert_eq!(second.cpu_history.len(), 1);
    assert!(second.memory.total_bytes > 0);
}
