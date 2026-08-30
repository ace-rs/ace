//! Platform-specific process primitives.

use std::process::ExitStatus;
use std::sync::atomic::{AtomicUsize, Ordering};

static SUPERVISED_CHILDREN: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug)]
pub struct ChildSupervision<'a> {
    count: &'a AtomicUsize,
}

impl ChildSupervision<'_> {
    fn enter(count: &AtomicUsize) -> ChildSupervision<'_> {
        count.fetch_add(1, Ordering::AcqRel);
        ChildSupervision { count }
    }

    #[cfg(test)]
    fn is_active(&self) -> bool {
        self.count.load(Ordering::Acquire) > 0
    }
}

impl Drop for ChildSupervision<'_> {
    fn drop(&mut self) {
        let previous = self.count.fetch_sub(1, Ordering::AcqRel);
        debug_assert!(previous > 0, "child supervision count underflow");
    }
}

pub fn begin_child_supervision() -> ChildSupervision<'static> {
    ChildSupervision::enter(&SUPERVISED_CHILDREN)
}

pub fn child_supervision_active() -> bool {
    SUPERVISED_CHILDREN.load(Ordering::Acquire) > 0
}

/// Return normally for success and otherwise terminate ACE with the child's
/// exit status. The supervised child has already exited before this boundary.
pub fn propagate_exit_status(status: ExitStatus) {
    if status.success() {
        return;
    }

    std::process::exit(exit_code(status));
}

#[cfg(unix)]
fn exit_code(status: ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt;

    status
        .code()
        .or_else(|| status.signal().map(|signal| 128 + signal))
        .unwrap_or(1)
}

#[cfg(windows)]
fn exit_code(status: ExitStatus) -> i32 {
    status.code().unwrap_or(1)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn child_exit_code_is_preserved() {
        let status = ExitStatus::from_raw(7 << 8);

        assert_eq!(exit_code(status), 7);
    }

    #[test]
    fn child_signal_uses_shell_exit_convention() {
        let status = ExitStatus::from_raw(15);

        assert_eq!(exit_code(status), 143);
    }

    #[test]
    fn child_supervision_guard_owns_the_active_state() {
        let count = AtomicUsize::new(0);

        let guard = ChildSupervision::enter(&count);
        assert!(guard.is_active());
        let second = ChildSupervision::enter(&count);
        assert_eq!(count.load(Ordering::Acquire), 2);

        drop(guard);
        assert!(second.is_active());
        drop(second);
        assert_eq!(count.load(Ordering::Acquire), 0);
    }
}
