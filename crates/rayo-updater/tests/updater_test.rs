use rayo_updater::{StartupAction, StateDir, UpdateConfig, UpdateMarker, handle_startup_marker};
use std::path::PathBuf;
use tempfile::TempDir;

fn test_state_dir() -> (TempDir, PathBuf) {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join(".rayo");
    (tmp, path)
}

#[test]
fn rate_limit_skips_when_recent() {
    let (_tmp, path) = test_state_dir();
    let state = StateDir::new(path).unwrap();

    state.write_last_check().unwrap();
    let elapsed = state.seconds_since_last_check();
    assert!(elapsed < 2, "Just wrote, should be <2s, got {elapsed}");
}

#[test]
fn rate_limit_allows_when_never_checked() {
    let (_tmp, path) = test_state_dir();
    let state = StateDir::new(path).unwrap();

    let elapsed = state.seconds_since_last_check();
    assert_eq!(elapsed, u64::MAX, "Never checked should return MAX");
}

#[test]
fn update_marker_round_trip() {
    let (_tmp, path) = test_state_dir();
    let state = StateDir::new(path).unwrap();

    let marker = UpdateMarker::new("0.1.0", "0.2.0");
    state.write_marker(&marker).unwrap();

    let read = state.read_marker().unwrap();
    assert_eq!(read.from_version, "0.1.0");
    assert_eq!(read.to_version, "0.2.0");
    assert_eq!(read.crash_count, 0);
}

#[test]
fn update_marker_clear() {
    let (_tmp, path) = test_state_dir();
    let state = StateDir::new(path).unwrap();

    let marker = UpdateMarker::new("0.1.0", "0.2.0");
    state.write_marker(&marker).unwrap();
    state.clear_marker().unwrap();

    assert!(state.read_marker().is_none());
}

#[test]
fn update_marker_clear_when_missing_is_ok() {
    let (_tmp, path) = test_state_dir();
    let state = StateDir::new(path).unwrap();

    // Should not error
    state.clear_marker().unwrap();
}

#[test]
fn marker_is_stale_when_version_mismatch() {
    let marker = UpdateMarker::new("0.1.0", "0.2.0");
    assert!(
        marker.is_stale("0.1.0"),
        "Running old version means update crashed"
    );
    assert!(
        !marker.is_stale("0.2.0"),
        "Running new version means update succeeded"
    );
}

#[test]
fn file_lock_prevents_second_acquisition() {
    let (_tmp, path) = test_state_dir();
    let state = StateDir::new(path).unwrap();

    let lock1 = state.try_lock();
    assert!(lock1.is_some(), "First lock should succeed");

    let lock2 = state.try_lock();
    assert!(lock2.is_none(), "Second lock should fail");

    drop(lock1);

    let lock3 = state.try_lock();
    assert!(lock3.is_some(), "Lock after drop should succeed");
}

#[test]
fn handle_startup_marker_none_when_no_marker() {
    let (_tmp, path) = test_state_dir();
    std::fs::create_dir_all(&path).unwrap();

    match handle_startup_marker(&path, "0.1.0") {
        StartupAction::None => {}
        _ => panic!("Expected None when no marker exists"),
    }
}

#[test]
fn handle_startup_marker_just_updated() {
    let (_tmp, path) = test_state_dir();
    let state = StateDir::new(path.clone()).unwrap();

    let marker = UpdateMarker::new("0.1.0", "0.2.0");
    state.write_marker(&marker).unwrap();

    // Running as v0.2.0 = success
    match handle_startup_marker(&path, "0.2.0") {
        StartupAction::JustUpdated { from, to } => {
            assert_eq!(from, "0.1.0");
            assert_eq!(to, "0.2.0");
        }
        _ => panic!("Expected JustUpdated"),
    }

    // Marker should be cleared
    assert!(state.read_marker().is_none());
}

#[test]
fn handle_startup_marker_rollback_detected() {
    let (_tmp, path) = test_state_dir();
    let state = StateDir::new(path.clone()).unwrap();

    let marker = UpdateMarker::new("0.1.0", "0.2.0");
    state.write_marker(&marker).unwrap();

    // Running as v0.1.0 = the update didn't stick (crash/rollback)
    match handle_startup_marker(&path, "0.1.0") {
        StartupAction::RollbackDetected {
            attempted_version, ..
        } => {
            assert_eq!(attempted_version, "0.2.0");
        }
        _ => panic!("Expected RollbackDetected"),
    }

    // Crash count should be incremented
    let updated = state.read_marker().unwrap();
    assert_eq!(updated.crash_count, 1);
}

#[test]
fn handle_startup_marker_crash_loop() {
    let (_tmp, path) = test_state_dir();
    let state = StateDir::new(path.clone()).unwrap();

    let mut marker = UpdateMarker::new("0.1.0", "0.2.0");
    marker.crash_count = 2; // Already crashed twice
    state.write_marker(&marker).unwrap();

    match handle_startup_marker(&path, "0.1.0") {
        StartupAction::CrashLoopDetected { version } => {
            assert_eq!(version, "0.2.0");
        }
        _ => panic!("Expected CrashLoopDetected"),
    }

    // Marker should be cleared (auto-update disabled)
    assert!(state.read_marker().is_none());
}

#[test]
fn config_struct_defaults() {
    let (_tmp, path) = test_state_dir();
    let config = UpdateConfig {
        disabled: false,
        check_interval_secs: 3600,
        github_owner: "manurueda".to_string(),
        github_repo: "rayo-browser".to_string(),
        app_name: "rayo-mcp".to_string(),
        state_dir: path,
    };
    assert!(!config.disabled);
    assert_eq!(config.check_interval_secs, 3600);
    assert_eq!(config.github_owner, "manurueda");
    assert_eq!(config.github_repo, "rayo-browser");
    assert_eq!(config.app_name, "rayo-mcp");
}

#[test]
fn config_disabled_flag() {
    let (_tmp, path) = test_state_dir();
    let config = UpdateConfig {
        disabled: true,
        check_interval_secs: 3600,
        github_owner: "manurueda".to_string(),
        github_repo: "rayo-browser".to_string(),
        app_name: "rayo-mcp".to_string(),
        state_dir: path,
    };
    assert!(config.disabled);
}

#[test]
fn config_custom_interval() {
    let (_tmp, path) = test_state_dir();
    let config = UpdateConfig {
        disabled: false,
        check_interval_secs: 300,
        github_owner: "manurueda".to_string(),
        github_repo: "rayo-browser".to_string(),
        app_name: "rayo-mcp".to_string(),
        state_dir: path,
    };
    assert_eq!(config.check_interval_secs, 300);
}

#[tokio::test]
async fn check_and_update_skips_when_disabled() {
    let (_tmp, path) = test_state_dir();
    let config = UpdateConfig {
        disabled: true,
        check_interval_secs: 3600,
        github_owner: "manurueda".to_string(),
        github_repo: "rayo-browser".to_string(),
        app_name: "rayo-mcp".to_string(),
        state_dir: path,
    };

    let result = rayo_updater::check_and_update(&config, "0.1.0").await;
    assert!(result.is_ok());
    match result.unwrap() {
        rayo_updater::UpdateOutcome::Skipped => {}
        _ => panic!("Expected Skipped when disabled"),
    }
}

#[tokio::test]
async fn check_and_update_skips_when_rate_limited() {
    let (_tmp, path) = test_state_dir();
    let state = StateDir::new(path.clone()).unwrap();
    state.write_last_check().unwrap();

    let config = UpdateConfig {
        disabled: false,
        check_interval_secs: 3600,
        github_owner: "manurueda".to_string(),
        github_repo: "rayo-browser".to_string(),
        app_name: "rayo-mcp".to_string(),
        state_dir: path,
    };

    let result = rayo_updater::check_and_update(&config, "0.1.0").await;
    assert!(result.is_ok());
    match result.unwrap() {
        rayo_updater::UpdateOutcome::Skipped => {}
        _ => panic!("Expected Skipped when rate limited"),
    }
}
