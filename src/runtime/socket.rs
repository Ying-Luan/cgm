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

use super::identity::{peer_uid, trusted_peer};

/// Bind before daemon startup performs any filesystem or database side effects.
///
/// # Returns
///
/// An exclusive listener on the fixed abstract address, retained for daemon lifetime.
pub(crate) fn bind() -> io::Result<UnixListener> {
    UnixListener::bind_addr(&SocketAddr::from_abstract_name(SOCKET_NAME)?)
}

/// Connect only to a peer owned by root or the caller.
///
/// # Returns
///
/// A connected stream after checking kernel peer credentials.
pub(crate) fn connect() -> io::Result<UnixStream> {
    let stream = connect_status()?;
    let uid = peer_uid(&stream)?;
    if !trusted_peer(uid, Uid::effective()) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Untrusted daemon owner",
        ));
    }

    Ok(stream)
}

/// Connect for a non-sensitive status request; the peer remains untrusted.
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
    fn peer_credentials_are_kernel_supplied() {
        let (a, b) = UnixStream::pair().unwrap();
        assert_eq!(peer_uid(&a).unwrap(), Uid::effective());
        assert_eq!(peer_uid(&b).unwrap(), Uid::effective());
    }

    #[test]
    fn root_does_not_trust_personal_daemons() {
        let root = Uid::from_raw(0);
        let user = Uid::from_raw(12345);
        assert!(!trusted_peer(user, root));
        assert!(trusted_peer(root, user));
        assert!(trusted_peer(user, user));
        assert!(!trusted_peer(user, Uid::from_raw(12346)));
    }
}
