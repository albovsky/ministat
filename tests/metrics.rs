use ministat::{
    metrics::{Sampler, disks},
    model::{Disk, Health, StatusView, now_unix},
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
fn disk_gauges_clear_expired_disk_readings_while_cpu_samples_remain_current() {
    let mut snapshot = Sampler::default().sample(
        vec![Disk::mounted(
            "Internal",
            "/System/Volumes/Data",
            1000,
            400,
            400,
        )],
        now_unix(),
        Health::default(),
    );
    let now = now_unix();
    let boot = snapshot.boot_time_unix;
    snapshot.sampled_at_unix = now;
    snapshot.disks_sampled_at_unix = now - 91;
    assert_eq!(
        StatusView::new(snapshot.clone(), now, boot).disk_gauge_percent(0),
        0.0
    );
    snapshot.disks_sampled_at_unix = now;
    let current = StatusView::new(snapshot.clone(), now, boot);
    assert_eq!(current.disk_gauge_percent(0), 60.0);
    assert_eq!(current.disk_gauge_percent(5), 0.0);
    snapshot.sampled_at_unix = now - 16;
    assert_eq!(
        StatusView::new(snapshot, now, boot).disk_gauge_percent(0),
        0.0
    );
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

#[test]
fn equivalent_spellings_of_a_mount_point_are_mounted() {
    for spelling in [
        "/System/Volumes/Data/",
        "/System/Volumes//Data",
        "/System/./Volumes/Data/",
    ] {
        let row = &disks(std::path::Path::new(spelling))[1];
        assert!(row.mounted, "{spelling} should be the Data volume");
        assert_eq!(row.mount, "/System/Volumes/Data");
        assert!(row.total_bytes.unwrap() > 0);
    }
}

#[test]
fn a_directory_inside_another_mount_is_still_not_mounted() {
    assert!(!disks(std::path::Path::new("/System/Volumes/Data/private/"))[1].mounted);
}
