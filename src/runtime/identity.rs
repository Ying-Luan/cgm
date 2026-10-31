//! Authentication of socket peers and requests.

use std::{io, os::unix::net::UnixStream};

use nix::{
    sys::socket::{getsockopt, sockopt::PeerCredentials},
    unistd::{Uid, User},
};

use crate::types::Request;

/// Obtain credentials supplied by the kernel at connection time.
///
/// # Arguments
///
/// * `stream` - Connected Unix socket whose peer credentials are queried.
///
/// # Returns
///
/// The peer's kernel-reported UID.
pub(crate) fn peer_uid(stream: &UnixStream) -> io::Result<Uid> {
    getsockopt(stream, PeerCredentials)
        .map(|credentials| Uid::from_raw(credentials.uid()))
        .map_err(io::Error::from)
}

/// Check whether a daemon peer is trusted by the caller.
///
/// # Arguments
///
/// * `peer` - Kernel-reported daemon UID.
/// * `caller` - Effective client UID.
///
/// # Returns
///
/// Whether the peer is root or has the same UID as the caller.
pub(super) fn trusted_peer(peer: Uid, caller: Uid) -> bool {
    peer.is_root() || peer == caller
}

/// Authenticate the kernel-provided UID and discard request-supplied identities.
///
/// # Arguments
///
/// * `req` - Decoded request; a submission's username is replaced after authentication.
/// * `peer` - Kernel-reported client UID.
/// * `owner` - Effective daemon owner UID.
///
/// # Returns
///
/// The authorized request and resolved requester name. Basic status bypasses account
/// lookup and returns an empty name; job-level permission checks remain with handlers.
pub(crate) fn authorize(
    mut req: Request,
    peer: Uid,
    owner: Uid,
) -> Result<(Request, String), String> {
    // Basic status does not use an account name.
    if matches!(req, Request::Status) {
        return Ok((req, String::new()));
    }
    if !owner.is_root() && peer != owner && !peer.is_root() && !matches!(req, Request::Status) {
        return Err("Permission denied: personal daemon belongs to another user".to_string());
    }
    if matches!(req, Request::Stop { .. } | Request::Delete { .. })
        && peer != owner
        && !peer.is_root()
    {
        return Err("Permission denied: daemon owner required".to_string());
    }
    let user = User::from_uid(peer)
        .map_err(|error| format!("Failed to resolve peer UID {}: {}", peer, error))?
        .ok_or_else(|| format!("No user found for peer UID {}", peer))?;

    // Existing job permissions use names, so reject ambiguous or privileged aliases.
    let named = User::from_name(&user.name)
        .map_err(|error| format!("Failed to resolve peer username: {}", error))?
        .ok_or_else(|| "Peer username not found".to_string())?;
    if named.uid != peer {
        return Err("Peer username does not uniquely identify its UID".to_string());
    }
    if let Request::Submit { job } = &mut req {
        job.username = user.name.clone();
    }
    Ok((req, user.name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{DeleteTarget, GpuSelection, JobRequest};

    #[test]
    fn basic_status_does_not_require_a_peer_account() {
        let (request, username) = authorize(
            Request::Status,
            Uid::from_raw(u32::MAX - 1),
            Uid::from_raw(12345),
        )
        .unwrap();
        assert!(matches!(request, Request::Status));
        assert!(username.is_empty());
    }

    #[test]
    fn foreign_business_requests_are_still_rejected() {
        let result = authorize(
            Request::List {
                username: "root".into(),
                limit: None,
            },
            Uid::from_raw(12346),
            Uid::from_raw(12345),
        );
        assert!(result.is_err());
    }

    #[test]
    fn root_can_request_personal_daemon_shutdown() {
        let (request, _) = authorize(
            Request::Stop { force: false },
            Uid::from_raw(0),
            Uid::from_raw(12345),
        )
        .unwrap();
        assert!(matches!(request, Request::Stop { force: false }));
    }

    #[test]
    fn system_daemon_requires_root_for_administration() {
        let other = Uid::from_raw(1001);
        let system = Uid::from_raw(0);
        assert!(authorize(Request::Stop { force: false }, other, system).is_err());
        assert!(
            authorize(
                Request::Delete {
                    target: DeleteTarget::AllTerminated
                },
                other,
                system
            )
            .is_err()
        );
    }

    #[test]
    fn peer_identity_replaces_request_usernames() {
        let uid = Uid::effective();
        let user = User::from_uid(uid).unwrap().unwrap();
        let (_, identity) = authorize(
            Request::List {
                username: "forged".into(),
                limit: None,
            },
            uid,
            uid,
        )
        .unwrap();
        assert_eq!(identity, user.name);

        let job = JobRequest {
            username: "forged".into(),
            allowed_gpus: GpuSelection::All,
            command: vec!["true".into()],
            dependencies: vec![],
            required_gpus: 1,
            envs: Default::default(),
            log_path: None,
            cwd: "/".into(),
        };
        let (request, identity) = authorize(Request::Submit { job }, uid, uid).unwrap();
        let Request::Submit { job } = request else {
            panic!("unexpected request")
        };
        assert_eq!(job.username, identity);
        assert_ne!(job.username, "forged");
    }

    #[test]
    fn unknown_business_uid_is_rejected() {
        let uid = Uid::from_raw(u32::MAX - 1);
        if User::from_uid(uid).unwrap().is_none() {
            assert!(
                authorize(
                    Request::List {
                        username: "root".into(),
                        limit: None,
                    },
                    uid,
                    Uid::from_raw(0)
                )
                .is_err()
            );
        }
    }
}
