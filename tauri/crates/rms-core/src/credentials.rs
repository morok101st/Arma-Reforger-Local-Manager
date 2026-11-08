//! Server-Passwörter gehören zu einem Profil und seiner Adresse: Sie werden nie für ein anderes Profil und
//! ohne Zustimmung des Nutzers nie für eine andere Adresse verwendet. Gleiche Einträge wie die WPF-App.

use std::io;

use windows::Win32::Foundation::{ERROR_NOT_FOUND, FILETIME};
use windows::Win32::Security::Credentials::{
    CRED_FLAGS, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW, CredDeleteW, CredFree, CredReadW, CredWriteW,
};
use windows::core::PWSTR;
use zeroize::Zeroize;

use crate::win::{pcwstr, wide};

pub trait CredentialStore: Send + Sync {
    fn read_password(&self, server_address: &str, profile_id: &str) -> io::Result<Option<String>>;
    fn save_password(&self, server_address: &str, profile_id: &str, password: &str) -> io::Result<()>;

    /// `true`, wenn ein Eintrag gelöscht wurde.
    fn delete_password(&self, server_address: &str, profile_id: &str) -> io::Result<bool>;
}

/// Windows-Anmeldeinformationsspeicher: vom Betriebssystem verschlüsselt, pro Benutzer.
#[derive(Debug, Default, Clone, Copy)]
pub struct WindowsCredentialStore;

pub fn target_name(server_address: &str, profile_id: &str) -> String {
    format!("ReforgerModSync:Server:{}:{profile_id}", server_address.trim())
}

fn is_not_found(error: &windows::core::Error) -> bool {
    error.code() == ERROR_NOT_FOUND.to_hresult()
}

fn to_io(error: windows::core::Error) -> io::Error {
    io::Error::other(error.message())
}

impl CredentialStore for WindowsCredentialStore {
    fn read_password(&self, server_address: &str, profile_id: &str) -> io::Result<Option<String>> {
        let target = wide(target_name(server_address, profile_id));
        let mut credential: *mut CREDENTIALW = std::ptr::null_mut();
        // SAFETY: Ziel ist nullterminiert, das Ergebnis wird unten mit CredFree freigegeben.
        if let Err(error) = unsafe { CredReadW(pcwstr(&target), CRED_TYPE_GENERIC, None, &mut credential) } {
            return if is_not_found(&error) { Ok(None) } else { Err(to_io(error)) };
        }

        // SAFETY: CredReadW hat einen gültigen Zeiger geliefert. Der Blob enthält UTF-16 ohne Nullzeichen.
        let password = unsafe {
            let blob = (*credential).CredentialBlob;
            let size = (*credential).CredentialBlobSize as usize / 2;
            let password =
                if blob.is_null() { String::new() } else { String::from_utf16_lossy(std::slice::from_raw_parts(blob.cast::<u16>(), size)) };
            CredFree(credential.cast());
            password
        };
        Ok(Some(password))
    }

    fn save_password(&self, server_address: &str, profile_id: &str, password: &str) -> io::Result<()> {
        let mut target = wide(target_name(server_address, profile_id));
        let mut user = wide("ReforgerModSync");
        let mut blob: Vec<u8> = password.encode_utf16().flat_map(u16::to_le_bytes).collect();
        let credential = CREDENTIALW {
            Flags: CRED_FLAGS(0),
            Type: CRED_TYPE_GENERIC,
            TargetName: PWSTR(target.as_mut_ptr()),
            Comment: PWSTR::null(),
            LastWritten: FILETIME::default(),
            CredentialBlobSize: u32::try_from(blob.len()).map_err(|_| io::Error::other("Passwort zu lang"))?,
            CredentialBlob: if blob.is_empty() { std::ptr::null_mut() } else { blob.as_mut_ptr() },
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            AttributeCount: 0,
            Attributes: std::ptr::null_mut(),
            TargetAlias: PWSTR::null(),
            UserName: PWSTR(user.as_mut_ptr()),
        };
        // SAFETY: Alle Zeiger zeigen auf lokale Puffer, die bis nach dem Aufruf leben.
        let result = unsafe { CredWriteW(&credential, 0) };
        blob.zeroize();
        result.map_err(to_io)
    }

    fn delete_password(&self, server_address: &str, profile_id: &str) -> io::Result<bool> {
        let target = wide(target_name(server_address, profile_id));
        // SAFETY: Ziel ist nullterminiert.
        match unsafe { CredDeleteW(pcwstr(&target), CRED_TYPE_GENERIC, None) } {
            Ok(()) => Ok(true),
            Err(error) if is_not_found(&error) => Ok(false),
            Err(error) => Err(to_io(error)),
        }
    }
}

/// Regeln für das Passwort eines Servers. Es gilt nur für seine Adresse: Für eine andere darf es nur mit
/// Zustimmung des Nutzers verwendet werden, sonst ginge es unbemerkt an einen fremden Server.
pub struct ProfilePasswords<'a> {
    store: &'a dyn CredentialStore,
}

impl<'a> ProfilePasswords<'a> {
    pub fn new(store: &'a dyn CredentialStore) -> Self {
        Self { store }
    }

    pub fn has_password(&self, address: Option<&str>, profile_id: &str) -> io::Result<bool> {
        match address {
            None => Ok(false),
            Some(address) => Ok(self.store.read_password(address, profile_id)?.is_some()),
        }
    }

    /// Muss der Nutzer entscheiden, ob das gespeicherte Passwort für die neue Adresse gelten soll? Nur wenn er
    /// kein neues eingibt, eines gespeichert ist und sich die Adresse tatsächlich ändert.
    pub fn needs_transfer_decision(
        has_saved_password: bool,
        saved_address: Option<&str>,
        new_address: Option<&str>,
        entered_password: &str,
    ) -> bool {
        entered_password.is_empty()
            && has_saved_password
            && matches!((saved_address, new_address), (Some(saved), Some(new)) if saved != new)
    }

    /// Speichert ein neu eingegebenes Passwort. Ein leeres lässt das gespeicherte unverändert.
    pub fn store_entered(&self, address: &str, profile_id: &str, entered_password: &str) -> io::Result<()> {
        if entered_password.is_empty() { Ok(()) } else { self.store.save_password(address, profile_id, entered_password) }
    }

    /// Die Adresse hat sich geändert. Mit `keep` wandert das Passwort zur neuen Adresse. Der Eintrag der alten
    /// Adresse verschwindet in jedem Fall, damit kein verwaistes Passwort zurückbleibt.
    pub fn move_to_new_address(&self, profile_id: &str, previous_address: &str, new_address: &str, keep: bool) -> io::Result<()> {
        if keep && let Some(mut password) = self.store.read_password(previous_address, profile_id)? {
            let saved = self.store.save_password(new_address, profile_id, &password);
            password.zeroize();
            saved?;
        }

        self.store.delete_password(previous_address, profile_id).map(|_| ())
    }
}
