use rms_core::addons::{AddonHealth, InstalledAddon, ModEntry, ModState, compare};

const ID: &str = "660E395AD366D1F7";

fn state(required_version: Option<&str>, installed: Option<InstalledAddon>) -> ModState {
    let result = compare(&[ModEntry::new(ID, "Mod", required_version)], installed.as_slice());
    assert_eq!(result.len(), 1);
    result[0].state
}

fn addon(version: &str, health: AddonHealth) -> InstalledAddon {
    InstalledAddon { id: ID.into(), name: Some("Mod".into()), version: Some(version.into()), folder: "ordner".into(), health }
}

fn ok(version: &str) -> Option<InstalledAddon> {
    Some(addon(version, AddonHealth::Ok))
}

#[test]
fn not_installed_is_missing() {
    assert_eq!(state(Some("1.0.0"), None), ModState::Missing);
}

#[test]
fn same_version_is_current() {
    assert_eq!(state(Some("1.0.0"), ok("1.0.0")), ModState::Current);
}

#[test]
fn older_local_version_is_outdated() {
    assert_eq!(state(Some("1.0.1"), ok("1.0.0")), ModState::Outdated);
}

#[test]
fn newer_local_version_is_not_called_outdated() {
    assert_eq!(state(Some("0.7.33"), ok("0.7.34")), ModState::Newer);
}

#[test]
fn version_parts_are_compared_as_numbers_not_as_text() {
    assert_eq!(state(Some("0.10.0"), ok("0.9.71")), ModState::Outdated);
}

#[test]
fn versions_of_different_length_are_compared() {
    assert_eq!(state(Some("1.0"), ok("1.0.1")), ModState::Newer);
}

#[test]
fn same_version_written_differently_is_current() {
    assert_eq!(state(Some("1.0"), ok("1.0.0")), ModState::Current);
}

#[test]
fn version_that_cannot_be_compared_is_just_different() {
    assert_eq!(state(Some("1.0.0-beta"), ok("1.0.0")), ModState::Different);
}

#[test]
fn modset_without_version_accepts_any_healthy_version() {
    assert_eq!(state(None, ok("1.0.0")), ModState::Current);
}

#[test]
fn unhealthy_addon_is_damaged_even_with_matching_version() {
    for health in [AddonHealth::Incomplete, AddonHealth::Corrupted, AddonHealth::UnknownMetadata] {
        assert_eq!(state(Some("1.0.0"), Some(addon("1.0.0", health))), ModState::Damaged, "{health:?}");
    }
}

#[test]
fn ids_match_regardless_of_case() {
    let result = compare(&[ModEntry::new(&ID.to_lowercase(), "Mod", None)], &[addon("1.0.0", AddonHealth::Ok)]);

    assert_eq!(result[0].state, ModState::Current);
}

#[test]
fn healthy_copy_wins_over_a_damaged_one_with_the_same_id() {
    let installed = [addon("1.0.0", AddonHealth::Incomplete), addon("1.0.0", AddonHealth::Ok)];

    let result = compare(&[ModEntry::new(ID, "Mod", Some("1.0.0"))], &installed);

    assert_eq!(result[0].state, ModState::Current);
}
