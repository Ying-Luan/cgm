//! Authentication of socket callers and requests.

use std::{io, os::unix::net::UnixStream};

use nix::{
    sys::socket::{getsockopt, sockopt::PeerCredentials},
    unistd::Uid,
};

use crate::types::Request;

/// Obtain credentials supplied by the kernel at connection time.
///
/// # Arguments
///
/// * `stream` - Connected Unix socket whose credentials are queried.
///
/// # Returns
///
/// The kernel-reported UID of the process on the other end of the socket.
pub(crate) fn connection_uid(stream: &UnixStream) -> io::Result<Uid> {
    getsockopt(stream, PeerCredentials)
        .map(|credentials| Uid::from_raw(credentials.uid()))
        .map_err(io::Error::from)
}

/// Check whether a daemon owner is trusted by the caller.
///
/// # Arguments
///
/// * `owner` - Kernel-reported daemon UID.
/// * `caller` - Effective client UID.
///
/// # Returns
///
/// Whether the owner is root or has the same UID as the caller.
pub(super) fn trusted_owner(owner: Uid, caller: Uid) -> bool {
    owner.is_root() || owner == caller
}

/// Authenticate the kernel-provided caller UID.
///
/// # Arguments
///
/// * `req` - Decoded request.
/// * `caller` - Kernel-reported caller UID.
/// * `owner` - Effective daemon owner UID.
///
/// # Returns
///
/// Success for an authorized caller. Basic status bypasses daemon
/// ownership checks; job-level permission checks remain with handlers.
pub(crate) fn authorize(req: &Request, caller: Uid, owner: Uid) -> Result<(), String> {
    // Basic status is readable across daemon owners.
    if matches!(req, Request::Status) {
        return Ok(());
    }
    if !owner.is_root() && caller != owner && !caller.is_root() {
        return Err("Permission denied: personal daemon belongs to another user".to_string());
    }
    if matches!(req, Request::Stop { .. } | Request::Delete { .. })
        && caller != owner
        && !caller.is_root()
    {
        return Err("Permission denied: daemon owner required".to_string());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::types::DeleteTarget;

    use super::*;

    #[test]
    fn basic_status_does_not_require_a_caller_account() {
        authorize(
            &Request::Status,
            Uid::from_raw(u32::MAX - 1),
            Uid::from_raw(12345),
        )
        .unwrap();
    }

    #[test]
    fn foreign_business_requests_are_still_rejected() {
        let result = authorize(
            &Request::List { limit: None },
            Uid::from_raw(12346),
            Uid::from_raw(12345),
        );
        assert!(result.is_err());
    }

    #[test]
    fn root_can_request_personal_daemon_shutdown() {
        authorize(
            &Request::Stop { force: false },
            Uid::from_raw(0),
            Uid::from_raw(12345),
        )
        .unwrap();
    }

    #[test]
    fn system_daemon_requires_root_for_administration() {
        let other = Uid::from_raw(1001);
        let system = Uid::from_raw(0);
        assert!(authorize(&Request::Stop { force: false }, other, system).is_err());
        assert!(
            authorize(
                &Request::Delete {
                    target: DeleteTarget::AllTerminated
                },
                other,
                system
            )
            .is_err()
        );
    }
}
