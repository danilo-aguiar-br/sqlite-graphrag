//! Platform hardening for the files that store the API key.
//!
//! Isolated on purpose: this is the only part of the config layer with
//! `unsafe` and platform `cfg`, so keeping it in one file makes the security
//! audit surface a single reviewable unit (GAP-SG-146).
//!
//! # Declared verification limit
//!
//! The Windows branch is verified ONLY on a real Windows host. There is no
//! CI in this project, so `restriction_installs_a_protected_single_ace_dacl`
//! has never executed anywhere; treat the Windows guarantee as reviewed code,
//! not as tested code.
//!
//! Extracting a platform-independent "pure" core out of it was considered and
//! rejected. Every value the function decides on — `GENERIC_ALL`, `SET_ACCESS`,
//! `NO_INHERITANCE`, `TRUSTEE_IS_SID`, `TRUSTEE_IS_USER`, and the
//! `DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION` pair — is
//! a `windows_sys` constant that does not exist off Windows, and the only
//! non-constant step (the two-call `GetTokenInformation` idiom that yields the
//! caller's SID) IS the system call. A host-independent test would therefore
//! have to re-declare those constants by hand and assert that the copy matches
//! itself, which verifies nothing about the ACL actually installed. The single
//! decision worth asserting — that the DACL is PROTECTED and carries exactly
//! one ACE — is only observable by reading the descriptor back from a real
//! file, which is precisely what the `cfg(windows)` test already does.
//!
//! # Declared scope limit: this module covers WRITING only
//!
//! Platform parity here is about the file this CLI writes. The READ side —
//! [`crate::config::load_config`] warning that an existing `config.toml` is
//! more permissive than `0o600` — has no Windows counterpart and is not
//! claimed to have one. Do not read the parity below as "loose permissions are
//! detected on every platform"; they are detected on Unix and restricted (not
//! detected) on Windows. That function's `# Declared limit` section carries
//! the reasoning.

/// Restricts `path` so that only the current user account may access it.
///
/// GAP-SG-144. On Unix the caller already applies `0o700`/`0o600`, so this is a
/// no-op there. On Windows nothing equivalent existed: the file holding the
/// OpenRouter API key simply inherited the parent directory's ACL, so any
/// process running as the same user — and, on a default profile, the
/// Administrators group — could read it.
///
/// Failure is returned rather than swallowed, and the two call sites in
/// [`save_config`] treat it DIFFERENTLY on purpose:
///
/// - the config FILE aborts the save, because it carries the OpenRouter API key
///   and persisting it unrestricted is the exact exposure this function exists
///   to prevent;
/// - the config DIRECTORY only warns, because the file's DACL is protected from
///   parent inheritance and therefore does not depend on the directory's own
///   restriction to hold.
///
/// # Known residual risk: TOCTOU window
///
/// The file DACL can only be applied after `NamedTempFile::persist` renames the
/// temporary into place, so between the rename and this call the file exists
/// with the inherited ACL. The directory restriction narrows that window, which
/// is the second reason it is applied at all. Closing the window entirely would
/// mean creating the file with its security descriptor already attached, which
/// is a rewrite of the atomic-persist path and is deliberately out of scope.
#[cfg(not(windows))]
pub(super) fn restrict_to_current_user(_path: &std::path::Path) -> std::io::Result<()> {
    Ok(())
}

/// Windows implementation of [`restrict_to_current_user`].
///
/// Builds a DACL containing a single ACE granting `GENERIC_ALL` to the SID of
/// the calling process, then installs it with
/// `PROTECTED_DACL_SECURITY_INFORMATION`. That flag is the load-bearing part:
/// it detaches the object from the parent directory's inheritable ACEs, which
/// is precisely the path by which other processes reach the key today.
#[cfg(windows)]
pub(super) fn restrict_to_current_user(path: &std::path::Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{CloseHandle, LocalFree, ERROR_SUCCESS, GENERIC_ALL};
    use windows_sys::Win32::Security::Authorization::{
        SetEntriesInAclW, SetNamedSecurityInfoW, EXPLICIT_ACCESS_W, NO_MULTIPLE_TRUSTEE,
        SET_ACCESS, SE_FILE_OBJECT, TRUSTEE_IS_SID, TRUSTEE_IS_USER, TRUSTEE_W,
    };
    use windows_sys::Win32::Security::{
        GetTokenInformation, TokenUser, ACL, DACL_SECURITY_INFORMATION, NO_INHERITANCE,
        PROTECTED_DACL_SECURITY_INFORMATION, TOKEN_QUERY, TOKEN_USER,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide.push(0);

    // SAFETY: every pointer handed to Win32 is either a stack local or a
    // heap buffer sized by the API's own two-call idiom. The process token and
    // the ACL allocated by `SetEntriesInAclW` are released on every exit path,
    // including the error paths.
    unsafe {
        let mut token = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err(std::io::Error::last_os_error());
        }

        // Two-call idiom: the first call fails with ERROR_INSUFFICIENT_BUFFER
        // and reports the size the token information needs.
        let mut needed: u32 = 0;
        GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &mut needed);
        if needed == 0 {
            let err = std::io::Error::last_os_error();
            CloseHandle(token);
            return Err(err);
        }
        let mut buf = vec![0u8; needed as usize];
        let ok = GetTokenInformation(
            token,
            TokenUser,
            buf.as_mut_ptr().cast(),
            needed,
            &mut needed,
        );
        CloseHandle(token);
        if ok == 0 {
            return Err(std::io::Error::last_os_error());
        }

        // `buf` outlives every use of `sid` below: the SID points into it.
        let sid = (*buf.as_ptr().cast::<TOKEN_USER>()).User.Sid;

        let access = EXPLICIT_ACCESS_W {
            grfAccessPermissions: GENERIC_ALL,
            grfAccessMode: SET_ACCESS,
            grfInheritance: NO_INHERITANCE,
            Trustee: TRUSTEE_W {
                pMultipleTrustee: std::ptr::null_mut(),
                MultipleTrusteeOperation: NO_MULTIPLE_TRUSTEE,
                TrusteeForm: TRUSTEE_IS_SID,
                TrusteeType: TRUSTEE_IS_USER,
                ptstrName: sid.cast(),
            },
        };

        let mut acl: *mut ACL = std::ptr::null_mut();
        let rc = SetEntriesInAclW(1, &access, std::ptr::null(), &mut acl);
        if rc != ERROR_SUCCESS {
            return Err(std::io::Error::from_raw_os_error(rc as i32));
        }

        // DO NOT drop PROTECTED_DACL_SECURITY_INFORMATION: without it the new
        // DACL merely COEXISTS with the parent directory's inheritable ACEs and
        // the exposure this function exists to close stays wide open. It looks
        // redundant next to the explicit ACE; it is not.
        let rc = SetNamedSecurityInfoW(
            wide.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            acl,
            std::ptr::null(),
        );
        LocalFree(acl.cast());
        if rc != ERROR_SUCCESS {
            return Err(std::io::Error::from_raw_os_error(rc as i32));
        }
    }

    Ok(())
}

/// GAP-SG-144: the config file holds the OpenRouter API key, so its access
/// restriction is part of the security contract, not a nicety.
#[cfg(test)]
mod restrict_permissions_tests {
    use super::restrict_to_current_user;

    #[test]
    #[cfg(not(windows))]
    fn restriction_is_a_noop_on_unix_where_chmod_already_applies() {
        // Unix reaches the same WRITE-side guarantee through `0o700`/`0o600`
        // in `save_config`; this must stay a cheap success so the shared call
        // site needs no `cfg` of its own. It says nothing about the read-side
        // permission check, which `load_config` declares as Unix-only.
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("config.toml");
        std::fs::write(&file, "x = 1").expect("write");
        assert!(restrict_to_current_user(&file).is_ok());
        assert!(restrict_to_current_user(dir.path()).is_ok());
    }

    #[test]
    #[cfg(windows)]
    fn restriction_installs_a_protected_single_ace_dacl() {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Foundation::{LocalFree, ERROR_SUCCESS};
        use windows_sys::Win32::Security::Authorization::{GetNamedSecurityInfoW, SE_FILE_OBJECT};
        use windows_sys::Win32::Security::{
            AclSizeInformation, GetAclInformation, GetSecurityDescriptorControl, ACL,
            ACL_SIZE_INFORMATION, DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR,
            SE_DACL_PROTECTED,
        };

        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("config.toml");
        std::fs::write(&file, "x = 1").expect("write");

        restrict_to_current_user(&file).expect("restriction applies");

        let mut wide: Vec<u16> = file.as_os_str().encode_wide().collect();
        wide.push(0);

        // SAFETY: read-back only; the descriptor returned by
        // `GetNamedSecurityInfoW` is freed before the assertions can unwind.
        unsafe {
            let mut dacl: *mut ACL = std::ptr::null_mut();
            let mut sd: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
            let rc = GetNamedSecurityInfoW(
                wide.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut dacl,
                std::ptr::null_mut(),
                &mut sd,
            );
            assert_eq!(rc, ERROR_SUCCESS, "reading the DACL back must succeed");

            let mut control: u16 = 0;
            let mut revision: u32 = 0;
            let ok = GetSecurityDescriptorControl(sd, &mut control, &mut revision);
            assert_ne!(ok, 0, "descriptor control must be readable");

            let mut info = ACL_SIZE_INFORMATION {
                AceCount: 0,
                AclBytesInUse: 0,
                AclBytesFree: 0,
            };
            let ok = GetAclInformation(
                dacl,
                (&mut info as *mut ACL_SIZE_INFORMATION).cast(),
                std::mem::size_of::<ACL_SIZE_INFORMATION>() as u32,
                AclSizeInformation,
            );
            assert_ne!(ok, 0, "ACL size information must be readable");

            LocalFree(sd.cast());

            // The inheritance break is the actual fix: without SE_DACL_PROTECTED
            // the parent directory's inheritable ACEs still grant access.
            assert_ne!(
                control & SE_DACL_PROTECTED,
                0,
                "DACL must be protected from parent-directory inheritance"
            );
            // Exactly one ACE: the current user. Any extra entry means another
            // trustee kept access to the API key.
            assert_eq!(info.AceCount, 1, "DACL must grant exactly one trustee");
        }
    }
}
