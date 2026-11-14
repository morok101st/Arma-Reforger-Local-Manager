mod common;

use std::fs;
use std::path::{Path, PathBuf};

use rms_core::addons::{AddonHealth, InstalledAddon, normalize_download_dir, resolve_mods_root, scan};

const ID: &str = "660E395AD366D1F7";

struct Addon<'a> {
    id: &'a str,
    file_content: &'a str,
    meta_size: u64,
    corrupted: bool,
    file_on_disk: bool,
}

impl Default for Addon<'_> {
    fn default() -> Self {
        Self { id: ID, file_content: "12345", meta_size: 5, corrupted: false, file_on_disk: true }
    }
}

impl Addon<'_> {
    /// Aufbau wie in echten meta-Dateien des Clients, inklusive veraltetem absolutem filePath.
    fn write(&self, root: &Path) -> PathBuf {
        let folder = root.join(format!("Test-Mod_{}", self.id));
        fs::create_dir_all(&folder).unwrap();
        let meta = format!(
            r#"{{"meta":{{"id":"{id}","name":"Test Mod","versions":[
              {{"version":"9.9.9","package":{{"files":[]}}}},
              {{"version":"0.2.4","corrupted":{corrupted},"package":{{"totalSize":{size},"files":[
                {{"isCorrupted":false,"isDownloaded":true,"size":{size},"filePath":"Z:\\weg\\data.pak","name":"data.pak"}}]}}}}
            ],"selectedRev":1}}}}"#,
            id = self.id,
            corrupted = self.corrupted,
            size = self.meta_size,
        );
        fs::write(folder.join("meta"), meta).unwrap();
        if self.file_on_disk {
            fs::write(folder.join("data.pak"), self.file_content).unwrap();
        }
        folder
    }
}

fn single_health(root: &Path) -> AddonHealth {
    let addons = scan(root);
    assert_eq!(addons.len(), 1);
    addons[0].health
}

#[test]
fn reads_id_name_and_selected_version() {
    let temp = common::temp_dir();
    let folder = Addon::default().write(temp.path());

    assert_eq!(
        scan(temp.path()),
        [InstalledAddon { id: ID.into(), name: Some("Test Mod".into()), version: Some("0.2.4".into()), folder, health: AddonHealth::Ok }]
    );
}

/// Der echte Client schreibt `meta` mit UTF-8-BOM.
#[test]
fn reads_meta_written_with_a_byte_order_mark() {
    let temp = common::temp_dir();
    let folder = Addon::default().write(temp.path());
    let meta = fs::read(folder.join("meta")).unwrap();
    fs::write(folder.join("meta"), [b"\xEF\xBB\xBF".as_slice(), &meta].concat()).unwrap();

    assert_eq!(single_health(temp.path()), AddonHealth::Ok);
}

#[test]
fn skips_folders_without_mod_id_like_saves() {
    let temp = common::temp_dir();
    fs::create_dir(temp.path().join("saves")).unwrap();

    assert!(scan(temp.path()).is_empty());
}

#[test]
fn missing_root_gives_empty_result() {
    assert!(scan(Path::new(r"Z:\gibt\es\nicht")).is_empty());
}

#[test]
fn missing_file_means_incomplete() {
    let temp = common::temp_dir();
    Addon { file_on_disk: false, ..Addon::default() }.write(temp.path());

    assert_eq!(single_health(temp.path()), AddonHealth::Incomplete);
}

#[test]
fn wrong_file_size_means_incomplete() {
    let temp = common::temp_dir();
    Addon { file_content: "123", ..Addon::default() }.write(temp.path());

    assert_eq!(single_health(temp.path()), AddonHealth::Incomplete);
}

#[test]
fn corrupted_flag_is_reported() {
    let temp = common::temp_dir();
    Addon { corrupted: true, ..Addon::default() }.write(temp.path());

    assert_eq!(single_health(temp.path()), AddonHealth::Corrupted);
}

#[test]
fn missing_or_foreign_meta_keeps_id_from_folder() {
    for meta in [None, Some("{ kaputt"), Some(r#"{"meta":{"id":"AAAAAAAAAAAAAAAA","versions":[]}}"#)] {
        let temp = common::temp_dir();
        let folder = temp.path().join(format!("Test-Mod_{ID}"));
        fs::create_dir(&folder).unwrap();
        if let Some(meta) = meta {
            fs::write(folder.join("meta"), meta).unwrap();
        }

        let addons = scan(temp.path());

        assert_eq!(addons.len(), 1, "{meta:?}");
        assert_eq!(addons[0].id, ID);
        assert_eq!(addons[0].health, AddonHealth::UnknownMetadata, "{meta:?}");
    }
}

#[test]
fn download_folder_maps_to_mods_root() {
    assert_eq!(resolve_mods_root(Some(r"D:\Mods")), PathBuf::from(r"D:\Mods\addons"));
}

#[test]
fn chosen_mod_folder_is_not_nested_twice() {
    let cases = [
        (Some(r"D:\Mods"), Some(r"D:\Mods")),
        (Some(r"D:\Mods\"), Some(r"D:\Mods")),
        (Some(r"D:\Mods\addons"), Some(r"D:\Mods")),
        (Some(r"D:\Mods\ADDONS\"), Some(r"D:\Mods")),
        (Some(r"D:\addons"), Some(r"D:\")),
        (Some("  "), None),
        (None, None),
    ];
    for (chosen, expected) in cases {
        assert_eq!(normalize_download_dir(chosen).as_deref(), expected, "{chosen:?}");
    }
}

#[test]
fn folder_with_the_mods_maps_to_itself() {
    assert_eq!(resolve_mods_root(Some(r"E:\Mods\addons")), PathBuf::from(r"E:\Mods\addons"));
}

#[test]
fn without_download_folder_the_client_default_is_used() {
    assert!(resolve_mods_root(None).ends_with(r"My Games\ArmaReforger\addons"));
}
