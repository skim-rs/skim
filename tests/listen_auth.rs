#![cfg(all(feature = "cli", feature = "listen"))]
#![allow(missing_docs)]

#[allow(dead_code, unused_macros)]
mod common;

use common::zellij::ZellijController;
use interprocess::bound_util::RefWrite as _;
use interprocess::local_socket::traits::Stream as _;
use interprocess::local_socket::{GenericNamespaced, Stream, ToNsName as _};
use rand::RngExt as _;
use skim::prelude::Action;
use std::io::{self, Write as _};

#[test]
fn listener_rejects_other_user() -> io::Result<()> {
    if std::env::var("SKIM_TEST_LISTENER_AUTH").as_deref() != Ok("1") {
        eprintln!("skipping listener rejection test: no second user configured");
        return Ok(());
    }

    let name = format!("sk-auth-{:016x}", rand::rng().random::<u64>());
    let mut zellij = ZellijController::new_named("listen-auth")?;
    zellij.start_sk(Some("printf 'item\\n'"), &["--listen", &name, "--header", "auth-ready"])?;
    zellij.until(|lines| lines.iter().any(|line| line.trim() == "auth-ready"))?;

    assert_other_user_rejected(&name)?;

    let stream = Stream::connect(name.to_ns_name::<GenericNamespaced>()?)?;
    let action = Action::SetHeader(Some("auth-allowed".to_owned()));
    writeln!(stream.as_write(), "{}", ron::to_string(&action).unwrap())?;
    zellij.until(|lines| lines.iter().any(|line| line.trim() == "auth-allowed"))
}

#[cfg(unix)]
fn assert_other_user_rejected(name: &str) -> io::Result<()> {
    use std::process::Command;

    #[cfg(not(target_os = "linux"))]
    {
        use std::os::unix::fs::{FileTypeExt as _, PermissionsExt as _};

        let path = std::path::Path::new("/tmp").join(name);
        assert!(path.symlink_metadata()?.file_type().is_socket());
        // Let the peer reach Skim's UID check instead of failing the filesystem check.
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o666))?;
    }

    let status = Command::new("sudo")
        .args(["-n", "-u", "nobody", "--", "python3", "-I", "-c"])
        .arg(
            r"
import os, socket, sys
assert os.geteuid() != int(sys.argv[2]), 'client UID is not different'
name = sys.argv[1]
address = '\0' + name if sys.platform.startswith('linux') else '/tmp/' + name
with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as client:
    client.settimeout(5)
    client.connect(address)
    # Without the check, Skim keeps reading because the write side is still open.
    try:
        assert client.recv(1) == b'', 'listener did not close the connection'
    except ConnectionResetError:
        pass
",
        )
        .arg(name)
        .arg(nix::unistd::geteuid().to_string())
        .status()
        .map_err(|err| io::Error::new(err.kind(), format!("could not start sudo: {err}")))?;
    assert!(status.success(), "other-user client failed: {status}");
    Ok(())
}

#[cfg(windows)]
fn assert_other_user_rejected(name: &str) -> io::Result<()> {
    use std::os::windows::io::{AsRawHandle as _, FromRawHandle as _, OwnedHandle};
    use std::ptr::null_mut;
    use windows_sys::Win32::Foundation::ERROR_ACCESS_DENIED;
    use windows_sys::Win32::Security::{
        ImpersonateLoggedOnUser, LOGON32_LOGON_NETWORK, LOGON32_PROVIDER_DEFAULT, LogonUserW, RevertToSelf,
    };

    let account = std::env::var("SKIM_TEST_OTHER_USER").expect("set SKIM_TEST_OTHER_USER");
    let password = std::env::var("SKIM_TEST_OTHER_PASSWORD").expect("set SKIM_TEST_OTHER_PASSWORD");
    assert_ne!(account, std::env::var("USERNAME").unwrap());
    let account: Vec<_> = account.encode_utf16().chain([0]).collect();
    let password: Vec<_> = password.encode_utf16().chain([0]).collect();
    let name = name.to_ns_name::<GenericNamespaced>()?;
    let mut token = null_mut();
    // The strings are NUL-terminated, and a successful logon returns an owned handle.
    unsafe {
        if LogonUserW(
            account.as_ptr(),
            [u16::from(b'.'), 0].as_ptr(),
            password.as_ptr(),
            LOGON32_LOGON_NETWORK,
            LOGON32_PROVIDER_DEFAULT,
            &raw mut token,
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
        let token = OwnedHandle::from_raw_handle(token);
        if ImpersonateLoggedOnUser(token.as_raw_handle()) == 0 {
            return Err(io::Error::last_os_error());
        }
        // Open the pipe on this thread, then revert before checking the result.
        let connection = Stream::connect(name);
        if RevertToSelf() == 0 {
            return Err(io::Error::last_os_error());
        }
        assert_eq!(
            connection.unwrap_err().raw_os_error(),
            Some(ERROR_ACCESS_DENIED.cast_signed())
        );
    }
    Ok(())
}
