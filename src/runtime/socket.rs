//! Linux abstract socket shared by clients and daemon startup.

use std::{
    io,
    os::{
        linux::net::SocketAddrExt,
        unix::net::{SocketAddr, UnixListener, UnixStream},
    },
};

use nix::unistd::Uid;

use crate::constants::SOCKET_NAME;

use super::identity::{connection_uid, trusted_owner};

/// Bind before daemon startup performs any filesystem or database side effects.
///
/// # Returns
///
/// An exclusive listener on the fixed abstract address, retained for daemon lifetime.
pub(crate) fn bind() -> io::Result<UnixListener> {
    UnixListener::bind_addr(&SocketAddr::from_abstract_name(SOCKET_NAME)?)
}

/// Connect only to a daemon owned by root or the caller.
///
/// # Returns
///
/// A connected stream after checking the daemon owner's credentials.
pub(crate) fn connect() -> io::Result<UnixStream> {
    let stream = connect_status()?;
    let owner = connection_uid(&stream)?;
    if !trusted_owner(owner, Uid::effective()) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Untrusted daemon owner",
        ));
    }

    Ok(stream)
}

/// Connect for a non-sensitive status request; the daemon owner remains untrusted.
///
/// # Returns
///
/// A stream without owner authentication, for basic status metadata only.
pub(crate) fn connect_status() -> io::Result<UnixStream> {
    UnixStream::connect_addr(&SocketAddr::from_abstract_name(SOCKET_NAME)?)
}

/// Check socket occupancy without sending any request or trusting its owner.
///
/// # Returns
///
/// Whether a connection succeeds.
pub(crate) fn occupied() -> bool {
    SocketAddr::from_abstract_name(SOCKET_NAME)
        .and_then(|address| UnixStream::connect_addr(&address))
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connection_uid_is_kernel_supplied() {
        let (a, b) = UnixStream::pair().unwrap();
        assert_eq!(connection_uid(&a).unwrap(), Uid::effective());
        assert_eq!(connection_uid(&b).unwrap(), Uid::effective());
    }

    #[test]
    fn root_does_not_trust_personal_daemons() {
        let root = Uid::from_raw(0);
        let user = Uid::from_raw(12345);
        assert!(!trusted_owner(user, root));
        assert!(trusted_owner(root, user));
        assert!(trusted_owner(user, user));
        assert!(!trusted_owner(user, Uid::from_raw(12346)));
    }
}
