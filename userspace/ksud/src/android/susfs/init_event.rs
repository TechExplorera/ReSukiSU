use std::time::Duration;

use anyhow::{Context, Result};
use log::{info, warn};
use prop_rs_android::{resetprop::ResetProp, sys_prop};

use crate::android::susfs::api;
use crate::android::susfs::config;
use crate::android::susfs::config::data::Data;

const USER_0_CE_AVAILABLE_PROP: &str = "sys.user.0.ce_available";
const CE_AVAILABLE_WAIT_TIMEOUT_SECS: u64 = 10 * 60;

enum CeAvailability {
    Available,
    Locked,
    Unknown,
}

pub fn on_boot_completed() {
    match user_0_ce_availability() {
        CeAvailability::Available => {
            apply_after_ce_available("user-0-ce-available-at-boot-completed");
        }
        CeAvailability::Locked => {
            wait_for_user_0_ce_available();
        }
        CeAvailability::Unknown => {
            if should_wait_for_user_0_ce_available() {
                wait_for_user_0_ce_available();
            } else {
                info!("{USER_0_CE_AVAILABLE_PROP} is unavailable on non-FBE device");
                apply_after_ce_available("boot-completed-without-ce-property");
            }
        }
    }
}

pub fn on_services() {
    // let config = config::read_config();

    // apply_sus_paths(&config);
    // apply_sus_maps(&config);
}

fn apply_sus_paths(config: &Data) {
    for sus_path in &config.sus_path.sus_path {
        if sus_path.trim().is_empty() {
            continue;
        }
        apply_sus_path_entry(&api::SusPathType::Normal, "sus_path", sus_path);
    }
    for sus_path_loop in &config.sus_path.sus_path_loop {
        if sus_path_loop.trim().is_empty() {
            continue;
        }
        apply_sus_path_entry(&api::SusPathType::Loop, "sus_path_loop", sus_path_loop);
    }
}

fn apply_sus_path_entry(path_type: &api::SusPathType, label: &str, path: &str) {
    if let Err(e) = api::add_sus_path(path_type, &path) {
        warn!("failed to add {label} '{path}': {e}");
    }
}

fn apply_sus_maps(config: &Data) {
    for sus_map in &config.sus_map {
        if sus_map.trim().is_empty() {
            continue;
        }
        if let Err(e) = api::add_sus_map(sus_map.as_str()) {
            warn!("failed to add sus_map '{sus_map}': {e}");
        }
    }
}

pub fn on_post_fs_data() {
    let config = config::read_config();

    if let Err(e) = api::set_uname(&config.common.spoof_version, &config.common.spoof_release) {
        warn!("failed to set uname: {e}");
    }

    if let Err(e) = api::enable_avc_log_spoofing(config.common.avc_spoofing.into()) {
        warn!("failed to enable avc log spoofing: {e}");
    }

    if let Err(e) = api::enable_log(config.common.enable_susfs_log.into()) {
        warn!("failed to enable susfs log: {e}");
    }

    if let Err(e) =
        api::hide_sus_mnts_for_non_su_procs(config.common.hide_sus_mnts_for_non_su_procs.into())
    {
        warn!("failed to hide sus mnts for non su procs: {e}");
    }

    // apply_sus_paths(&config);

    apply_sus_kstat_additions(&config);
}

pub fn on_post_mount() {
    let config = config::read_config();

    // apply_sus_paths(&config);
    // apply_sus_maps(&config);

    apply_kstat_updates(&config);
}

fn apply_after_ce_available(reason: &str) {
    let config = config::read_config();

    info!("applying susfs CE-sensitive entries for {reason}");
    apply_sus_paths(&config);
    apply_sus_maps(&config);
    apply_sus_kstat_additions(&config);
    apply_kstat_updates(&config);
}

fn apply_sus_kstat_additions(config: &Data) {
    for sus_kstat in &config.kstat.sus_kstat {
        if sus_kstat.trim().is_empty() {
            continue;
        }
        if let Err(e) = api::add_sus_kstat(sus_kstat.as_str()) {
            warn!("failed to add sus_kstat '{sus_kstat}': {e}");
        }
    }
    for statically in &config.kstat.statically {
        if statically.path.trim().is_empty() {
            continue;
        }
        if let Err(e) = api::add_sus_kstat_statically(
            &statically.path,
            &statically.ino,
            &statically.dev,
            &statically.nlink,
            &statically.size,
            &statically.atime,
            &statically.atime_nsec,
            &statically.mtime,
            &statically.mtime_nsec,
            &statically.ctime,
            &statically.ctime_nsec,
            &statically.blocks,
            &statically.blksize,
        ) {
            warn!(
                "failed to add sus_kstat_statically '{}': {}",
                statically.path, e
            );
        }
    }
}

fn apply_kstat_updates(config: &Data) {
    for update_kstat in &config.kstat.update_kstat {
        if update_kstat.trim().is_empty() {
            continue;
        }
        if let Err(e) = api::update_sus_kstat(update_kstat.as_str()) {
            warn!("failed to update sus_kstat '{update_kstat}': {e}");
        }
    }
    for full_clone in &config.kstat.full_clone {
        if full_clone.trim().is_empty() {
            continue;
        }
        if let Err(e) = api::update_sus_kstat_full_clone(full_clone.as_str()) {
            warn!("failed to update sus_kstat_full_clone '{full_clone}': {e}");
        }
    }
}

fn user_0_ce_availability() -> CeAvailability {
    match crate::android::utils::getprop(USER_0_CE_AVAILABLE_PROP)
        .as_deref()
        .map(str::trim)
    {
        Some(value) if is_true_property_value(value) => CeAvailability::Available,
        Some(value) if is_false_property_value(value) => CeAvailability::Locked,
        _ => CeAvailability::Unknown,
    }
}

fn is_true_property_value(value: &str) -> bool {
    value == "1" || value.eq_ignore_ascii_case("true")
}

fn is_false_property_value(value: &str) -> bool {
    value == "0" || value.eq_ignore_ascii_case("false")
}

fn should_wait_for_user_0_ce_available() -> bool {
    crate::android::utils::getprop("ro.crypto.type")
        .as_deref()
        .is_some_and(|value| value.eq_ignore_ascii_case("file"))
}

fn wait_for_user_0_ce_available() {
    match crate::android::utils::create_daemon(false) {
        Ok(true) => {}
        Ok(false) => return,
        Err(e) => {
            warn!("failed to daemonize susfs CE availability watcher: {e}");
            return;
        }
    }

    let exit_code = match wait_for_user_0_ce_available_inner() {
        Ok(true) => {
            apply_after_ce_available("user-0-ce-available");
            0
        }
        Ok(false) => 0,
        Err(e) => {
            warn!("failed to wait for {USER_0_CE_AVAILABLE_PROP}: {e}");
            1
        }
    };
    unsafe {
        libc::_exit(exit_code);
    }
}

fn wait_for_user_0_ce_available_inner() -> Result<bool> {
    sys_prop::init().context("Failed to initialize system property API")?;
    let rp = resetprop();

    info!("waiting for {USER_0_CE_AVAILABLE_PROP}");
    loop {
        match user_0_ce_availability() {
            CeAvailability::Available => return Ok(true),
            CeAvailability::Locked | CeAvailability::Unknown => {}
        }

        let current_value = crate::android::utils::getprop(USER_0_CE_AVAILABLE_PROP);
        let changed = rp
            .wait(
                USER_0_CE_AVAILABLE_PROP,
                current_value.as_deref(),
                Some(Duration::from_secs(CE_AVAILABLE_WAIT_TIMEOUT_SECS)),
            )
            .context("wait for user 0 CE availability failed")?;
        if !changed {
            warn!("timed out waiting for {USER_0_CE_AVAILABLE_PROP}");
            return Ok(false);
        }
    }
}

const fn resetprop() -> ResetProp {
    ResetProp {
        skip_svc: true,
        persistent: false,
        persist_only: false,
        verbose: false,
        show_context: false,
    }
}
