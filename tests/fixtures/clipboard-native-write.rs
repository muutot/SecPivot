#![allow(dead_code, unused_imports, non_snake_case, non_upper_case_globals)]

// The complete production platform module uses this fake Win32 import root.
// Handles identify live-owned registry allocations; GlobalLock returns their
// still-owned aligned storage. No released memory or OS clipboard is accessed.
mod windows_sys {
    pub mod Win32 {
        pub mod Foundation {
            pub use crate::api::GlobalFree;
            pub type HGLOBAL = *mut std::ffi::c_void;
            pub type HWND = *mut std::ffi::c_void;
        }
        pub mod System {
            pub mod Memory {
                pub use crate::api::{GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock};
                pub const GMEM_MOVEABLE: u32 = 2;
                pub const GMEM_ZEROINIT: u32 = 64;
            }
            pub mod DataExchange {
                pub use crate::api::{
                    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard,
                    RegisterClipboardFormatW, SetClipboardData,
                };
            }
        }
        pub mod UI {
            pub mod WindowsAndMessaging {
                pub use crate::api::{CreateWindowExW, DestroyWindow};
                pub const HWND_MESSAGE: super::super::Foundation::HWND =
                    (-3isize) as super::super::Foundation::HWND;
            }
        }
    }
}

mod api {
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::ffi::c_void;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Failure {
        None,
        Owner,
        Allocate,
        Lock,
        Open,
        Empty,
        Publish,
    }
    struct Allocation {
        data: Vec<u64>,
        len: usize,
        locks: usize,
        transferred: bool,
    }
    pub struct State {
        pub failure: Failure,
        allocations: BTreeMap<usize, Allocation>,
        next: usize,
        pub open: bool,
        pub owner_live: bool,
        owner: usize,
        pub opens: usize,
        pub closes: usize,
        pub empties: usize,
        pub publications: usize,
        pub creates: usize,
        pub destroys: usize,
        pub freed_zeroed: Vec<bool>,
        pub posted: Option<Vec<u8>>,
        pub posted_format: Option<u32>,
        pub clipboard_changed: bool,
    }
    impl State {
        pub fn new(failure: Failure) -> Self {
            Self {
                failure,
                allocations: BTreeMap::new(),
                next: 1,
                open: false,
                owner_live: false,
                owner: 0,
                opens: 0,
                closes: 0,
                empties: 0,
                publications: 0,
                creates: 0,
                destroys: 0,
                freed_zeroed: vec![],
                posted: None,
                posted_format: None,
                clipboard_changed: false,
            }
        }
        pub fn owned_allocations(&self) -> usize {
            self.allocations.values().filter(|v| !v.transferred).count()
        }
    }
    thread_local! { static STATE: RefCell<State> = RefCell::new(State::new(Failure::None)); }
    pub fn reset(failure: Failure) {
        STATE.with(|s| *s.borrow_mut() = State::new(failure));
    }
    pub fn inspect(f: impl FnOnce(&State)) {
        STATE.with(|s| f(&s.borrow()));
    }
    fn with<T>(f: impl FnOnce(&mut State) -> T) -> T {
        STATE.with(|s| f(&mut s.borrow_mut()))
    }
    pub unsafe fn CreateWindowExW(
        _: u32,
        class: *const u16,
        _: *const u16,
        _: u32,
        _: i32,
        _: i32,
        _: i32,
        _: i32,
        parent: *mut c_void,
        _: *mut c_void,
        _: *mut c_void,
        _: *const c_void,
    ) -> *mut c_void {
        assert!(!class.is_null());
        assert_eq!(parent as isize, -3, "writer owner must be message-only");
        with(|s| {
            s.creates += 1;
            if s.failure == Failure::Owner {
                return std::ptr::null_mut();
            }
            assert!(!s.owner_live);
            s.owner_live = true;
            123usize as *mut c_void
        })
    }
    pub unsafe fn DestroyWindow(owner: *mut c_void) -> i32 {
        assert_eq!(owner as usize, 123);
        with(|s| {
            assert!(s.owner_live);
            assert!(!s.open);
            s.owner_live = false;
            s.destroys += 1;
            1
        })
    }
    pub unsafe fn OpenClipboard(owner: *mut c_void) -> i32 {
        with(|s| {
            s.opens += 1;
            if s.failure == Failure::Open {
                return 0;
            }
            assert!(!s.open);
            if !owner.is_null() {
                assert!(s.owner_live);
            }
            s.open = true;
            s.owner = owner as usize;
            1
        })
    }
    pub unsafe fn CloseClipboard() -> i32 {
        with(|s| {
            assert!(s.open);
            assert!(s
                .allocations
                .values()
                .all(|v| !v.transferred || v.locks == 0));
            s.open = false;
            s.closes += 1;
            1
        })
    }
    pub unsafe fn EmptyClipboard() -> i32 {
        with(|s| {
            assert!(s.open);
            s.empties += 1;
            if s.failure == Failure::Empty {
                return 0;
            }
            s.clipboard_changed = true;
            s.posted = None;
            1
        })
    }
    pub unsafe fn GlobalAlloc(flags: u32, len: usize) -> *mut c_void {
        assert_eq!(flags, 66);
        assert!(len <= 128, "fixture never creates resource pressure");
        with(|s| {
            if s.failure == Failure::Allocate {
                return std::ptr::null_mut();
            }
            let id = s.next;
            s.next += 1;
            s.allocations.insert(
                id,
                Allocation {
                    data: vec![0; len.div_ceil(8)],
                    len,
                    locks: 0,
                    transferred: false,
                },
            );
            id as *mut c_void
        })
    }
    pub unsafe fn GlobalLock(handle: *mut c_void) -> *mut c_void {
        with(|s| {
            if s.failure == Failure::Lock {
                return std::ptr::null_mut();
            }
            let v = s
                .allocations
                .get_mut(&(handle as usize))
                .expect("live allocation");
            v.locks += 1;
            v.data.as_mut_ptr().cast()
        })
    }
    pub unsafe fn GlobalUnlock(handle: *mut c_void) -> i32 {
        with(|s| {
            let v = s
                .allocations
                .get_mut(&(handle as usize))
                .expect("live allocation");
            assert!(v.locks > 0);
            v.locks -= 1;
            0
        })
    }
    pub unsafe fn GlobalFree(handle: *mut c_void) -> *mut c_void {
        with(|s| {
            let v = s
                .allocations
                .remove(&(handle as usize))
                .expect("live allocation");
            assert!(!v.transferred, "successful publication belongs to OS");
            assert_eq!(v.locks, 0);
            let bytes = std::slice::from_raw_parts(v.data.as_ptr().cast::<u8>(), v.len);
            s.freed_zeroed.push(bytes.iter().all(|v| *v == 0));
            std::ptr::null_mut()
        })
    }
    pub unsafe fn GlobalSize(handle: *mut c_void) -> usize {
        with(|s| {
            s.allocations
                .get(&(handle as usize))
                .expect("live allocation")
                .len
        })
    }
    pub unsafe fn SetClipboardData(format: u32, handle: *mut c_void) -> *mut c_void {
        with(|s| {
            assert!(s.open);
            s.publications += 1;
            // Win32 explicitly rejects SetClipboardData after OpenClipboard(NULL)
            // followed by EmptyClipboard. This is contract simulation, not a
            // claim that an actual native clipboard was exercised.
            if s.owner == 0 || s.failure == Failure::Publish {
                return std::ptr::null_mut();
            }
            let v = s
                .allocations
                .get_mut(&(handle as usize))
                .expect("live allocation");
            v.transferred = true;
            s.posted =
                Some(std::slice::from_raw_parts(v.data.as_ptr().cast::<u8>(), v.len).to_vec());
            s.posted_format = Some(format);
            handle
        })
    }
    pub unsafe fn RegisterClipboardFormatW(_: *const u16) -> u32 {
        100
    }
    pub unsafe fn GetClipboardData(_: u32) -> *mut c_void {
        panic!("no read in writer fixture")
    }
}

// PRODUCTION_CLIPBOARD

#[cfg(test)]
mod tests {
    use super::{
        api::{self, Failure},
        clipboard,
    };
    fn copy(binary: bool) -> Result<(), String> {
        if binary {
            clipboard::write_clipboard_bytes("synthetic-format", &[0, 255, 42, 0])
        } else {
            clipboard::write_clipboard_text("synthetic🔑")
        }
    }
    fn failure_case(binary: bool, failure: Failure, before_open: bool, before_publish: bool) {
        api::reset(failure);
        assert!(
            copy(binary).is_err(),
            "{failure:?} must not publish success"
        );
        api::inspect(|s| {
            assert_eq!(
                s.owned_allocations(),
                0,
                "{failure:?} must free owned allocation"
            );
            assert!(!s.open);
            assert!(!s.owner_live);
            assert!(
                s.freed_zeroed.iter().all(|v| *v),
                "owned copied bytes must wipe before free"
            );
            assert!(s.posted.is_none());
            if before_open {
                assert_eq!(s.opens, 0);
                assert!(!s.clipboard_changed);
            }
            if before_publish {
                assert_eq!(s.publications, 0);
            }
        });
    }
    fn successful_case(binary: bool) {
        api::reset(Failure::None);
        assert!(copy(binary).is_ok());
        api::inspect(|s| {
            assert_eq!(s.owned_allocations(), 0);
            assert!(!s.open);
            assert!(!s.owner_live);
            assert_eq!(
                (
                    s.creates,
                    s.destroys,
                    s.opens,
                    s.closes,
                    s.empties,
                    s.publications
                ),
                (1, 1, 1, 1, 1, 1)
            );
            assert!(
                s.freed_zeroed.is_empty(),
                "transferred memory must not free"
            );
            let text: Vec<u8> = "synthetic🔑"
                .encode_utf16()
                .chain([0])
                .flat_map(u16::to_le_bytes)
                .collect();
            let expected: &[u8] = if binary { &[0, 255, 42, 0] } else { &text };
            assert_eq!(s.posted.as_deref(), Some(expected));
            assert_eq!(s.posted_format, Some(if binary { 100 } else { 13 }));
        });
    }
    #[test]
    fn text_success_uses_live_owner_and_transfers_once() {
        successful_case(false);
    }
    #[test]
    fn binary_success_uses_live_owner_and_transfers_once() {
        successful_case(true);
    }
    #[test]
    fn text_owner_failure_never_opens_clipboard() {
        failure_case(false, Failure::Owner, true, true);
    }
    #[test]
    fn binary_owner_failure_never_opens_clipboard() {
        failure_case(true, Failure::Owner, true, true);
    }
    #[test]
    fn text_allocation_failure_preserves_clipboard() {
        failure_case(false, Failure::Allocate, true, true);
    }
    #[test]
    fn binary_allocation_failure_preserves_clipboard() {
        failure_case(true, Failure::Allocate, true, true);
    }
    #[test]
    fn text_lock_failure_frees_zero_initialized_storage() {
        failure_case(false, Failure::Lock, true, true);
    }
    #[test]
    fn binary_lock_failure_frees_zero_initialized_storage() {
        failure_case(true, Failure::Lock, true, true);
    }
    #[test]
    fn text_open_failure_releases_prepared_bytes_and_owner() {
        failure_case(false, Failure::Open, false, true);
    }
    #[test]
    fn binary_open_failure_releases_prepared_bytes_and_owner() {
        failure_case(true, Failure::Open, false, true);
    }
    #[test]
    fn text_empty_failure_never_attempts_publication() {
        failure_case(false, Failure::Empty, false, true);
    }
    #[test]
    fn binary_empty_failure_never_attempts_publication() {
        failure_case(true, Failure::Empty, false, true);
    }
    #[test]
    fn text_publication_failure_wipes_and_frees_owned_bytes() {
        failure_case(false, Failure::Publish, false, false);
    }
    #[test]
    fn binary_publication_failure_wipes_and_frees_owned_bytes() {
        failure_case(true, Failure::Publish, false, false);
    }
}
