//! Linux audit netlink source for AC-88 Landlock denial records.

#![cfg(target_os = "linux")]

use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

use super::landlock_audit::AuditRecordSource;

const MAX_NETLINK_PACKETS_PER_READ: usize = 256;

#[derive(Debug)]
pub struct AuditNetlinkSource {
    fd: OwnedFd,
}

impl AuditNetlinkSource {
    pub fn new() -> io::Result<Self> {
        const NETLINK_AUDIT: libc::c_int = 9;
        const AUDIT_NLGRP_READLOG: u32 = 1;

        let fd = unsafe {
            libc::socket(
                libc::AF_NETLINK,
                libc::SOCK_RAW | libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK,
                NETLINK_AUDIT,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }

        let fd = unsafe { OwnedFd::from_raw_fd(fd) };
        let mut addr: libc::sockaddr_nl = unsafe { std::mem::zeroed() };
        addr.nl_family = libc::AF_NETLINK as libc::sa_family_t;
        addr.nl_pid = 0;
        addr.nl_groups = 1u32 << (AUDIT_NLGRP_READLOG - 1);

        let result = unsafe {
            libc::bind(
                fd.as_raw_fd(),
                &addr as *const libc::sockaddr_nl as *const libc::sockaddr,
                std::mem::size_of::<libc::sockaddr_nl>() as libc::socklen_t,
            )
        };
        if result < 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(Self { fd })
    }
}

impl AuditRecordSource for AuditNetlinkSource {
    fn read_available_records(&mut self) -> io::Result<Vec<String>> {
        let mut buffer = [0u8; 65536];
        let mut records = Vec::new();
        for _ in 0..MAX_NETLINK_PACKETS_PER_READ {
            let n = unsafe {
                libc::recv(
                    self.fd.as_raw_fd(),
                    buffer.as_mut_ptr().cast(),
                    buffer.len(),
                    libc::MSG_DONTWAIT,
                )
            };
            if n < 0 {
                let err = io::Error::last_os_error();
                if err.kind() == io::ErrorKind::WouldBlock {
                    break;
                }
                return Err(err);
            }
            if n == 0 {
                break;
            }
            records.extend(netlink_payloads_to_records(&buffer[..n as usize]));
        }
        Ok(records)
    }
}

fn netlink_payloads_to_records(packet: &[u8]) -> Vec<String> {
    const NLMSG_ALIGN_TO: usize = 4;

    fn align(len: usize) -> usize {
        (len + NLMSG_ALIGN_TO - 1) & !(NLMSG_ALIGN_TO - 1)
    }

    let header_len = std::mem::size_of::<libc::nlmsghdr>();
    let mut offset = 0usize;
    let mut records = Vec::new();
    while packet.len().saturating_sub(offset) >= header_len {
        let header =
            unsafe { std::ptr::read_unaligned(packet[offset..].as_ptr().cast::<libc::nlmsghdr>()) };
        let message_len = header.nlmsg_len as usize;
        if message_len < header_len || offset + message_len > packet.len() {
            break;
        }
        let payload = &packet[offset + header_len..offset + message_len];
        if let Ok(text) = std::str::from_utf8(payload) {
            let text = text.trim_end_matches('\0').trim();
            if !text.is_empty() {
                records.push(text.to_string());
            }
        }
        offset += align(message_len);
    }
    records
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_audit_netlink_payload_from_nlmsghdr_frame() {
        let payload = b"type=LANDLOCK_DENIED msg=audit(1777298358.207:45): pid=4242 syscall=openat name=\"/tmp/ac88\" requested=write_file\0";
        let header_len = std::mem::size_of::<libc::nlmsghdr>();
        let mut frame = Vec::with_capacity(header_len + payload.len());
        let header = libc::nlmsghdr {
            nlmsg_len: (header_len + payload.len()) as u32,
            nlmsg_type: 0,
            nlmsg_flags: 0,
            nlmsg_seq: 7,
            nlmsg_pid: 0,
        };
        let header_bytes = unsafe {
            std::slice::from_raw_parts((&header as *const libc::nlmsghdr).cast::<u8>(), header_len)
        };
        frame.extend_from_slice(header_bytes);
        frame.extend_from_slice(payload);

        let records = netlink_payloads_to_records(&frame);

        assert_eq!(records.len(), 1);
        assert!(records[0].contains("type=LANDLOCK_DENIED"));
        assert!(records[0].contains("pid=4242"));
        assert!(!records[0].ends_with('\0'));
    }
}
