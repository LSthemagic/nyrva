//! Private local metadata and OS-released locks. Never changes provider credentials.
use crate::storage;
use std::{
    fs::{self, File, OpenOptions},
    path::Path,
};

pub(crate) fn directory(path: &Path) -> Result<(), String> {
    storage::private_dir(path)?;
    owner_only(path, true)
}
pub(crate) fn read(path: &Path, limit: usize) -> Result<Option<Vec<u8>>, String> {
    storage::check_path(path)?;
    if let Ok(m) = fs::symlink_metadata(path) {
        if !m.is_file() {
            return Err("expected a regular local file".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if m.nlink() != 1 {
                return Err("local state must not be hard-linked".into());
            }
        }
    }
    storage::read_optional(path, limit)
}
pub(crate) fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or("missing local parent directory")?;
    directory(parent)?;
    storage::atomic_write(path, bytes)?;
    owner_only(path, false)
}
pub(crate) fn lock(path: &Path) -> Result<File, String> {
    directory(path.parent().ok_or("missing lock directory")?)?;
    read(path, 16)?;
    let mut options = OpenOptions::new();
    options.create(true).read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options
        .open(path)
        .map_err(|_| "cannot open local operation lock")?;
    owner_only(path, false)?;
    file.try_lock()
        .map_err(|_| "another local operation is already running")?;
    Ok(file)
}
#[cfg(unix)]
fn owner_only(path: &Path, directory: bool) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(
        path,
        fs::Permissions::from_mode(if directory { 0o700 } else { 0o600 }),
    )
    .map_err(|_| "cannot restrict local state to its owner".into())
}
#[cfg(windows)]
fn owner_only(path: &Path, directory: bool) -> Result<(), String> {
    use std::{os::windows::ffi::OsStrExt, ptr};
    use windows_sys::Win32::{
        Foundation::{CloseHandle, LocalFree},
        Security::{
            Authorization::{
                ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
                SDDL_REVISION_1,
            },
            GetTokenInformation, SetFileSecurityW, TokenUser, DACL_SECURITY_INFORMATION,
            PROTECTED_DACL_SECURITY_INFORMATION, TOKEN_QUERY, TOKEN_USER,
        },
        System::Threading::{GetCurrentProcess, OpenProcessToken},
    };
    // All pointers below refer to aligned owned buffers or documented Win32 allocations.
    // The protected DACL has a single owner ACE. Children inherit it for private dirs.
    unsafe {
        let mut token = ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err("cannot identify private state owner".into());
        }
        let mut size = 0;
        GetTokenInformation(token, TokenUser, ptr::null_mut(), 0, &mut size);
        let mut storage = vec![0usize; (size as usize).div_ceil(std::mem::size_of::<usize>())];
        let ok = GetTokenInformation(
            token,
            TokenUser,
            storage.as_mut_ptr().cast(),
            size,
            &mut size,
        );
        CloseHandle(token);
        if ok == 0 {
            return Err("cannot read private state owner".into());
        }
        let user = &*(storage.as_ptr().cast::<TOKEN_USER>());
        let mut sid = ptr::null_mut();
        if ConvertSidToStringSidW(user.User.Sid, &mut sid) == 0 {
            return Err("cannot encode private state owner".into());
        }
        let mut len = 0;
        while *sid.add(len) != 0 {
            len += 1;
        }
        let sid_text = String::from_utf16_lossy(std::slice::from_raw_parts(sid, len));
        LocalFree(sid.cast());
        let inheritance = if directory { "OICI" } else { "" };
        let sddl: Vec<u16> = format!("D:P(A;{inheritance};FA;;;{sid_text})\0")
            .encode_utf16()
            .collect();
        let mut descriptor = ptr::null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            ptr::null_mut(),
        ) == 0
        {
            return Err("cannot build private state permissions".into());
        }
        let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let ok = SetFileSecurityW(
            path.as_ptr(),
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            descriptor,
        );
        LocalFree(descriptor);
        if ok == 0 {
            return Err("cannot restrict local state to its owner".into());
        }
    }
    Ok(())
}
#[cfg(not(any(unix, windows)))]
fn owner_only(_: &Path, _: bool) -> Result<(), String> {
    Err("private state permissions unsupported on this platform".into())
}
