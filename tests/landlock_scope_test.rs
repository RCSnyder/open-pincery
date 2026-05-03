//! AC-87 / Slice G0e: Landlock IPC scoping proof.
//!
//! The policy unit tests pin that both scope bits are sent to
//! `pincery-init`. This integration test exercises the observable
//! IPC behavior on a real ABI >= 6 kernel: a host abstract socket
//! and host process are reachable before sandboxing, but the
//! restricted process cannot connect or signal after Landlock scopes
//! are installed.

#![cfg(target_os = "linux")]

use std::io::{Read, Write};
use std::os::fd::{FromRawFd, IntoRawFd, RawFd};
use std::os::linux::net::SocketAddrExt;
use std::os::unix::net::{SocketAddr, UnixListener, UnixStream};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use open_pincery::config::{ResolvedSandboxMode, SandboxMode};
use open_pincery::runtime::sandbox::init_policy::{SandboxInitPolicy, LANDLOCK_SCOPE_ALL};
use open_pincery::runtime::sandbox::preflight::{KernelProbe, RealKernelProbe, LANDLOCK_ABI_FLOOR};
use open_pincery::runtime::sandbox::{
    bwrap::RealSandbox, ExecResult, SandboxProfile, ShellCommand, ToolExecutor,
};

fn command_available(name: &str) -> bool {
    Command::new("sh")
        .args(["-c", &format!("command -v {name} >/dev/null 2>&1")])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn strict_landlock_floor_available() -> bool {
    let probe = RealKernelProbe;
    probe
        .landlock_abi()
        .map(|abi| abi >= LANDLOCK_ABI_FLOOR)
        .unwrap_or(false)
}

fn preconditions_met() -> bool {
    if std::env::var_os("OPEN_PINCERY_SKIP_REAL_BWRAP").is_some() {
        return false;
    }
    if !command_available("bwrap") {
        eprintln!("skipping: bwrap not on PATH");
        return false;
    }
    if !command_available("socat") {
        eprintln!("skipping: socat not on PATH");
        return false;
    }
    if !open_pincery::runtime::sandbox::landlock_layer::landlock_supported() {
        eprintln!("skipping: kernel does not support landlock (need >= 5.13)");
        return false;
    }
    if !strict_landlock_floor_available() {
        eprintln!("skipping: Landlock ABI below AC-87 strict floor {LANDLOCK_ABI_FLOOR}");
        return false;
    }
    std::env::set_var("PINCERY_INIT_BIN_PATH", env!("CARGO_BIN_EXE_pincery-init"));
    true
}

fn enforce_sandbox() -> RealSandbox {
    RealSandbox::new(ResolvedSandboxMode {
        mode: SandboxMode::Enforce,
        allow_unsafe: false,
    })
}

fn scope_profile() -> SandboxProfile {
    SandboxProfile {
        env_allowlist: vec!["PATH".into()],
        deny_net: false,
        timeout: Duration::from_secs(10),
        cwd: None,
        cgroup: None,
        seccomp: false,
        landlock: true,
    }
}

fn make_policy_memfd(bytes: &[u8]) -> RawFd {
    let name = c"pincery-init-ac87-policy";
    let fd = unsafe { libc::memfd_create(name.as_ptr(), 0) };
    assert!(
        fd >= 0,
        "memfd_create failed: {}",
        std::io::Error::last_os_error()
    );

    let mut file = unsafe { std::fs::File::from_raw_fd(fd) };
    file.write_all(bytes).expect("write policy bytes");
    let raw = file.into_raw_fd();
    let rc = unsafe { libc::lseek(raw, 0, libc::SEEK_SET) };
    assert_eq!(
        rc,
        0,
        "lseek to 0 failed: {}",
        std::io::Error::last_os_error()
    );
    raw
}

fn init_policy_with_scopes(user_argv: Vec<String>) -> SandboxInitPolicy {
    let cur_uid = unsafe { libc::geteuid() };
    let cur_gid = unsafe { libc::getegid() };
    SandboxInitPolicy {
        landlock_rx_paths: vec![
            PathBuf::from("/usr"),
            PathBuf::from("/bin"),
            PathBuf::from("/lib"),
            PathBuf::from("/lib64"),
            PathBuf::from("/etc"),
            PathBuf::from("/sys"),
        ],
        landlock_rwx_paths: vec![PathBuf::from("/proc"), PathBuf::from("/tmp")],
        landlock_scopes: LANDLOCK_SCOPE_ALL,
        landlock_restrict_flags: 0,
        seccomp_bpf: Vec::new(),
        target_uid: cur_uid,
        target_gid: cur_gid,
        require_fully_enforced: true,
        user_argv,
    }
}

fn run_pincery_init_with_policy(policy: SandboxInitPolicy) -> std::process::Output {
    let bytes = policy.to_bytes().expect("serialize policy");
    let policy_fd = make_policy_memfd(&bytes);

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_pincery-init"));
    cmd.args(["--policy-fd", "3", "--", "/bin/sh", "-c", "unused"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    unsafe {
        cmd.pre_exec(move || {
            if libc::dup2(policy_fd, 3) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }

    cmd.output().expect("spawn pincery-init")
}

fn spawn_signal_target() -> Child {
    Command::new("/bin/sh")
        .args(["-c", "trap '' TERM; sleep 30"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn signal target")
}

fn assert_host_can_signal(pid: u32) {
    let status = Command::new("/bin/kill")
        .args(["-0", &pid.to_string()])
        .status()
        .expect("host kill -0");
    assert!(status.success(), "host should be able to signal pid {pid}");
}

fn spawn_abstract_echo_server(name: &[u8]) -> mpsc::Sender<()> {
    let addr = SocketAddr::from_abstract_name(name).expect("abstract socket address");
    let listener = UnixListener::bind_addr(&addr).expect("bind host abstract socket");
    listener
        .set_nonblocking(true)
        .expect("set abstract listener nonblocking");

    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    thread::spawn(move || loop {
        if stop_rx.try_recv().is_ok() {
            break;
        }
        match listener.accept() {
            Ok((mut stream, _addr)) => {
                let mut buf = [0u8; 64];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(b"ok");
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(_) => break,
        }
    });
    stop_tx
}

fn assert_host_can_connect(name: &[u8]) {
    let addr = SocketAddr::from_abstract_name(name).expect("abstract socket address");
    let mut stream = UnixStream::connect_addr(&addr).expect("host connect to abstract socket");
    stream.write_all(b"ping").expect("host write ping");
    let mut response = [0u8; 2];
    stream
        .read_exact(&mut response)
        .expect("host read response");
    assert_eq!(&response, b"ok");
}

#[tokio::test]
async fn landlock_scope_blocks_host_abstract_socket() {
    if !preconditions_met() {
        return;
    }

    let socket_name = format!("open-pincery-ac87-{}", std::process::id());
    let stop = spawn_abstract_echo_server(socket_name.as_bytes());
    assert_host_can_connect(socket_name.as_bytes());

    let script = format!(
        "printf probe | socat -t 1 - ABSTRACT-CONNECT:{socket_name} >/dev/null 2>&1 && echo abstract=connected || echo abstract=blocked"
    );
    let result = enforce_sandbox()
        .run(&ShellCommand::new(script), &scope_profile())
        .await;
    let _ = stop.send(());

    match result {
        ExecResult::Ok {
            stdout,
            stderr,
            exit_code,
            ..
        } => {
            assert_eq!(
                exit_code, 0,
                "abstract socket probe failed; stderr={stderr:?}"
            );
            assert!(
                stdout.contains("abstract=blocked"),
                "AC-87 must block host abstract socket connections: {stdout:?}"
            );
            assert!(
                !stdout.contains("abstract=connected"),
                "host abstract socket unexpectedly reachable from sandbox: {stdout:?}"
            );
        }
        other => panic!("expected Ok abstract socket probe, got {other:?}"),
    }
}

#[test]
fn landlock_scope_blocks_host_signal_probe() {
    if !preconditions_met() {
        return;
    }

    let mut target = spawn_signal_target();
    let target_pid = target.id();
    assert_host_can_signal(target_pid);

    let script = format!(
        "/bin/kill -0 {target_pid} 2>/tmp/ac87-signal.err && echo signal=allowed || {{ rc=$?; echo signal=blocked:$rc; cat /tmp/ac87-signal.err; }}"
    );
    let policy = init_policy_with_scopes(vec!["/bin/sh".into(), "-c".into(), script]);
    let output = run_pincery_init_with_policy(policy);
    let _ = target.kill();
    let _ = target.wait();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "pincery-init signal proof failed: status={:?} stdout={stdout:?} stderr={stderr:?}",
        output.status
    );
    assert!(
        stdout.contains("signal=blocked:"),
        "AC-87 must block signals to outside-domain processes: stdout={stdout:?} stderr={stderr:?}"
    );
    assert!(
        !stdout.contains("signal=allowed"),
        "host process unexpectedly signalable after Landlock scopes: stdout={stdout:?}"
    );
    assert!(
        stdout.to_ascii_lowercase().contains("operation not permitted")
            || stdout.to_ascii_lowercase().contains("not permitted"),
        "signal denial should be EPERM, not an invisible PID or shell error: stdout={stdout:?} stderr={stderr:?}"
    );
}
