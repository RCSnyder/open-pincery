//! AC-53 / AC-65: cgroup v2 resource caps (cpu, memory, pids).
//!
//! Stub — populated in Slice A2b.4. Will create a transient
//! cgroup under `/sys/fs/cgroup/open-pincery.slice/<exec-id>`,
//! apply `cpu.max`, `memory.max`, `pids.max` from the invocation
//! budget, and attach the spawned process before exec.
