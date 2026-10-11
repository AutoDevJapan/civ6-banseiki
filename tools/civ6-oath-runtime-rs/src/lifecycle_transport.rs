//! Parent-owned, unnamed pagefile transport for the finite lifecycle observer.
//! The parent initializes once before duplication. Both views use atomic loads;
//! a snapshot is a point-in-time observation, not a drain/lifetime certificate.
use crate::lifecycle_trace::LifecycleTrace;
use crate::runtime::OathRuntimeConfig;
use std::{io, ptr};
use windows_sys::Win32::Foundation::{
    CloseHandle, DuplicateHandle, DUPLICATE_SAME_ACCESS, HANDLE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::System::Memory::{
    CreateFileMappingW, MapViewOfFile, UnmapViewOfFile, VirtualQuery, FILE_MAP_READ,
    FILE_MAP_WRITE, MEMORY_BASIC_INFORMATION, MEMORY_MAPPED_VIEW_ADDRESS, MEM_MAPPED,
    PAGE_READWRITE,
};
use windows_sys::Win32::System::SystemInformation::{GetSystemInfo, SYSTEM_INFO};
use windows_sys::Win32::System::Threading::GetCurrentProcess;

pub const MAPPING_SIZE: u32 = std::mem::size_of::<LifecycleTrace>() as u32;

/// `config.size` remains 48; the shared envelope is exactly 64 bytes.
/// `mapping_handle` is already duplicated into the receiving process. It must
/// refer to a fresh SharedTrace and remain open until that process exits.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SharedLifecycleConfig {
    pub config: OathRuntimeConfig,
    pub mapping_handle: u64,
    pub mapping_size: u32,
    pub reserved: u32,
}
const _: () = assert!(std::mem::size_of::<SharedLifecycleConfig>() == 64);
const _: () = assert!(std::mem::offset_of!(SharedLifecycleConfig, mapping_handle) == 48);

/// Owns the parent handle and view; dropping it never closes a target's handle.
/// The parent's view remains readable after the target process terminates.
pub struct SharedTrace {
    handle: HANDLE,
    view: MappedTrace,
}
// A mapping handle/view is not thread-affine. Moving its sole owner preserves
// its lifetime; every writable field in snapshots is accessed atomically.
unsafe impl Send for SharedTrace {}
impl SharedTrace {
    pub fn new() -> io::Result<Self> {
        let handle = unsafe {
            CreateFileMappingW(
                INVALID_HANDLE_VALUE,
                ptr::null(),
                PAGE_READWRITE,
                0,
                MAPPING_SIZE,
                ptr::null(),
            )
        };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        // No other view/handle has been published: the sole initialization.
        let view = match unsafe { MappedTrace::map(handle, MAPPING_SIZE) } {
            Ok(view) => view,
            Err(e) => {
                unsafe { CloseHandle(handle) };
                return Err(e);
            }
        };
        unsafe { ptr::write(view.as_ptr(), LifecycleTrace::new()) };
        Ok(Self { handle, view })
    }

    /// Loads every concurrently writable field atomically and serializes the
    /// existing v1 wire layout. No raw byte copy of the atomic storage is made.
    pub fn snapshot(&self) -> Vec<u8> {
        self.view.trace().snapshot()
    }

    /// Duplicate into a process opened with PROCESS_DUP_HANDLE. The returned
    /// value is meaningful only there, and is retained there until process exit,
    /// including when install fails. It is not a handle owned by this object.
    #[allow(clippy::not_unsafe_ptr_arg_deref)] // HANDLE is an OS token, never dereferenced here.
    pub fn duplicate_into(&self, target: HANDLE) -> io::Result<u64> {
        let mut duplicate = ptr::null_mut();
        if unsafe {
            DuplicateHandle(
                GetCurrentProcess(),
                self.handle,
                target,
                &mut duplicate,
                0,
                0,
                DUPLICATE_SAME_ACCESS,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(duplicate as u64)
    }

    /// Borrowed parent-view pointer, invalid after this owner is dropped.
    /// Dereferencing or publishing it requires the caller to preserve its lifetime.
    pub fn as_ptr(&self) -> *mut LifecycleTrace {
        self.view.as_ptr()
    }
}
impl Drop for SharedTrace {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.handle) };
        // MappedTrace drops the view independently of the mapping handle.
    }
}

/// A local view of a borrowed mapping handle. Mapping never initializes/resets
/// bytes; it may be dropped only before gateway publication.
pub struct MappedTrace {
    view: MEMORY_MAPPED_VIEW_ADDRESS,
}
impl MappedTrace {
    /// # Safety
    /// The borrowed handle must identify a SharedTrace-compatible mapping. Its
    /// sole creator may initialize exclusively before sharing or taking a trace
    /// reference. After sharing, header/reserved fields must remain immutable
    /// and all mutable fields must only be accessed atomically. The
    /// caller must not unmap this view while any gateway/reference can use it.
    pub unsafe fn map(handle: HANDLE, size: u32) -> io::Result<Self> {
        if handle.is_null() || handle == INVALID_HANDLE_VALUE || size != MAPPING_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid lifecycle mapping",
            ));
        }
        // Map the whole object so an oversized section cannot masquerade as
        // the requested prefix. Windows rounds section extents to page size;
        // the envelope still requires the exact 32816-byte protocol length.
        let view = unsafe { MapViewOfFile(handle, FILE_MAP_READ | FILE_MAP_WRITE, 0, 0, 0) };
        if view.Value.is_null() {
            return Err(io::Error::last_os_error());
        }
        let mapped = Self { view };
        let mut system: SYSTEM_INFO = unsafe { std::mem::zeroed() };
        let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
        unsafe { GetSystemInfo(&mut system) };
        let page = system.dwPageSize as usize;
        if page == 0
            || unsafe { VirtualQuery(view.Value, &mut info, std::mem::size_of_val(&info)) }
                != std::mem::size_of_val(&info)
            || info.Type != MEM_MAPPED
            || info.RegionSize != (size as usize).div_ceil(page) * page
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "lifecycle mapping extent differs",
            ));
        }
        Ok(mapped)
    }

    pub fn as_ptr(&self) -> *mut LifecycleTrace {
        self.view.Value.cast()
    }
    pub fn trace(&self) -> &LifecycleTrace {
        unsafe { &*self.as_ptr() }
    }

    /// Retain the view until process termination even if later publication or
    /// restoration fails. This does not accept native retirement/quiescence.
    pub(crate) fn retain(self) -> &'static LifecycleTrace {
        let p = self.as_ptr();
        std::mem::forget(self);
        unsafe { &*p }
    }
}
impl Drop for MappedTrace {
    fn drop(&mut self) {
        unsafe { UnmapViewOfFile(self.view) };
    }
}
