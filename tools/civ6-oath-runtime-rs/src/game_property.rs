//! Native adapter for the official Game property container.
//!
//! The native string method makes its own owned copy and sends its own
//! container notification. No Variant, allocator, or HKS frame is fabricated.
//! This module is not called by the diagnostic exports. A normal bootstrap,
//! save/load callback and native holding/lifecycle admission are still needed.

use crate::contribution_save::{OfficialGameProperty, SaveError, MAX_PROPERTY_BYTES, PROPERTY_KEY};
use crate::native::{c_void, copy_current};
use crate::preflight::{ImageMemory, PropertySourcePlan};
use std::ffi::CString;
use std::marker::PhantomData;
use std::rc::Rc;
use windows_sys::Win32::System::Threading::GetCurrentThreadId;

const GAME_GLOBAL: u64 = 0xb8aa68;
const PROPERTY_OFFSET: u64 = 0x1b8;
const PROPERTY_TABLE: u64 = 0xabf080;
const MAX_VALUE: usize = MAX_PROPERTY_BYTES;

type Key = unsafe extern "system" fn(*mut c_void, *const i8) -> u32;
type Lookup = unsafe extern "system" fn(*mut c_void, u32) -> *const c_void;
type StringSetter = unsafe extern "system" fn(*mut c_void, u32, *const i8, u32) -> *const c_void;
type Cow = unsafe extern "system" fn(*mut c_void) -> *mut c_void;

struct CurrentImage;
impl ImageMemory for CurrentImage {
    fn copy(&mut self, address: u64, data: &mut [u8]) -> Result<(), &'static str> {
        let mut read = 0;
        let ok = unsafe {
            windows_sys::Win32::System::Diagnostics::Debug::ReadProcessMemory(
                windows_sys::Win32::System::Threading::GetCurrentProcess(),
                address as *const c_void,
                data.as_mut_ptr().cast(),
                data.len(),
                &mut read,
            )
        };
        if ok == 0 || read != data.len() {
            return Err("native property image is unreadable");
        }
        Ok(())
    }
}

fn word(address: u64) -> Result<u64, SaveError> {
    copy_current::<8>(address)
        .map(u64::from_le_bytes)
        .ok_or(SaveError::PropertyUnavailable)
}

/// A borrowed accessor confined to one admitted native holding interval.
/// It is neither Send nor Sync and stores no Variant or payload pointer.
pub struct NativeGameProperty<'hold> {
    plan: &'hold PropertySourcePlan,
    base: u64,
    game: u64,
    thread: u32,
    _thread_bound: PhantomData<Rc<()>>,
}

/// Borrow the actual current Game, use its properties, then discard the borrow.
/// No production caller has been wired yet; diagnostics must not call this.
///
/// # Safety
/// The caller must already hold the actual native Game/property/dictionary
/// lifetime and mutation interval on this thread for the entire closure,
/// including nested native notifications. It must exclude teardown, reload,
/// receiver replacement/ABA and conflicting property access. The loaded Core
/// and source file must be independently pinned to the supplied plan and base.
/// Source matching, pointer readability and a repeated global value do not
/// establish this contract. Calling this from a remote RPC thread, a quantity
/// getter, or an arbitrary copied trace is not an admitted interval.
pub unsafe fn with_borrowed_current<R>(
    plan: &PropertySourcePlan,
    base: u64,
    use_property: impl for<'hold> FnOnce(&mut NativeGameProperty<'hold>) -> R,
) -> Result<R, SaveError> {
    plan.validate_image(base, &mut CurrentImage)
        .map_err(|_| SaveError::Identity)?;
    let game = word(base.checked_add(GAME_GLOBAL).ok_or(SaveError::Identity)?)?;
    if game == 0 || !game.is_multiple_of(8) {
        return Err(SaveError::PropertyUnavailable);
    }
    let mut property = NativeGameProperty {
        plan,
        base,
        game,
        thread: unsafe { GetCurrentThreadId() },
        _thread_bound: PhantomData,
    };
    property.container()?;
    let result = use_property(&mut property);
    property.container()?;
    Ok(result)
}

impl NativeGameProperty<'_> {
    fn container(&self) -> Result<u64, SaveError> {
        if unsafe { GetCurrentThreadId() } != self.thread
            || word(self.base + GAME_GLOBAL)? != self.game
        {
            return Err(SaveError::Identity);
        }
        let container = self
            .game
            .checked_add(PROPERTY_OFFSET)
            .ok_or(SaveError::Identity)?;
        if word(container)? != self.base + PROPERTY_TABLE {
            return Err(SaveError::Identity);
        }
        Ok(container)
    }

    fn dictionary(&self, container: u64) -> Result<(u64, Key), SaveError> {
        // The audited +18/+20/+28 methods are the same leaf returning this+20.
        let dictionary = word(container + 0x20)?;
        if dictionary == 0 || !dictionary.is_multiple_of(8) {
            return Err(SaveError::PropertyUnavailable);
        }
        let method = self
            .plan
            .dictionary_key_method(self.base, word(dictionary)?, &mut CurrentImage)
            .map_err(|_| SaveError::Identity)?;
        Ok((dictionary, unsafe {
            std::mem::transmute::<usize, Key>(method as usize)
        }))
    }

    fn key(key: &str) -> Result<CString, SaveError> {
        // This accessor owns only the ledger property, never arbitrary fields.
        if key != PROPERTY_KEY {
            return Err(SaveError::Identity);
        }
        CString::new(key).map_err(|_| SaveError::Corrupt)
    }

    fn read_value(&self, value: u64) -> Result<String, SaveError> {
        let header = copy_current::<24>(value).ok_or(SaveError::PropertyUnavailable)?;
        let (pointer, length) = string_layout(&header)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| SaveError::Bounds)?;
        bytes.resize(length, 0);
        CurrentImage
            .copy(pointer, &mut bytes)
            .map_err(|_| SaveError::PropertyUnavailable)?;
        if bytes.pop() != Some(0) || bytes.contains(&0) || !bytes.is_ascii() {
            return Err(SaveError::Corrupt);
        }
        self.container()?;
        String::from_utf8(bytes).map_err(|_| SaveError::Corrupt)
    }
}

fn string_layout(header: &[u8; 24]) -> Result<(u64, usize), SaveError> {
    let tag = u32::from_le_bytes(header[..4].try_into().unwrap());
    let pointer = u64::from_le_bytes(header[8..16].try_into().unwrap());
    let count = u32::from_le_bytes(header[16..20].try_into().unwrap());
    let flags = u32::from_le_bytes(header[20..24].try_into().unwrap());
    let length = (flags & 0x00ff_ffff) as usize;
    if tag != 5 || count != 1 || pointer == 0 || flags & 0xff00_0000 != 0x2100_0000 {
        return Err(SaveError::Corrupt);
    }
    if !(2..=MAX_VALUE + 1).contains(&length) {
        return Err(SaveError::Bounds);
    }
    Ok((pointer, length))
}

impl OfficialGameProperty for NativeGameProperty<'_> {
    fn read(&mut self, key: &str) -> Result<Option<String>, SaveError> {
        let key = Self::key(key)?;
        let container = self.container()?;
        let (dictionary, lookup_key) = self.dictionary(container)?;
        // Unlike the setter, the getter does not register absent keys.
        let id = unsafe { lookup_key(dictionary as *mut c_void, key.as_ptr()) };
        self.container()?;
        let lookup: Lookup = unsafe { std::mem::transmute((self.base + 0x99f5c0) as usize) };
        let value = unsafe { lookup(container as *mut c_void, id) } as u64;
        self.container()?;
        if value == 0 {
            return Ok(None);
        }
        Ok(Some(self.read_value(value)?))
    }

    fn write(&mut self, key: &str, value: &str) -> Result<(), SaveError> {
        let key = Self::key(key)?;
        if value.is_empty() || !value.is_ascii() || value.len() > MAX_VALUE || value.contains('\0')
        {
            return Err(SaveError::Bounds);
        }
        // Owned Rust input remains alive until the native method has copied it.
        let value = CString::new(value).map_err(|_| SaveError::Corrupt)?;
        self.container()?;
        let cow: Cow = unsafe { std::mem::transmute((self.base + 0x72a920) as usize) };
        // This is the formal Game setter's COW path at Game+1A8, not an extra
        // call on City+10C0 to fetch production statistics.
        let writable = unsafe { cow((self.game + 0x1a8) as *mut c_void) } as u64;
        let container = self.container()?;
        if writable != container {
            return Err(SaveError::Identity);
        }
        let (dictionary, _) = self.dictionary(container)?;
        let intern: Key = unsafe { std::mem::transmute((self.base + 0x9a3f10) as usize) };
        let id = unsafe { intern(dictionary as *mut c_void, key.as_ptr()) };
        self.container()?;
        let setter: StringSetter = unsafe { std::mem::transmute((self.base + 0x9a07b0) as usize) };
        // Native vtable+E8: tag5, count1, NUL-inclusive width, owned copy;
        // its +B0 implementation publishes the container's +38 notification.
        let result = unsafe {
            setter(
                container as *mut c_void,
                id,
                value.as_ptr(),
                value.as_bytes_with_nul().len() as u32,
            )
        };
        self.container()?;
        // A native notification can invalidate the returned borrowed Variant.
        // Re-lookup the owned property rather than dereference that pointer.
        if result.is_null()
            || self.read(PROPERTY_KEY)?.as_deref().map(str::as_bytes) != Some(value.as_bytes())
        {
            return Err(SaveError::WriteNotConfirmed);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn string_header(length: u32) -> [u8; 24] {
        let mut header = [0; 24];
        header[..4].copy_from_slice(&5u32.to_le_bytes());
        header[8..16].copy_from_slice(&0x1000u64.to_le_bytes());
        header[16..20].copy_from_slice(&1u32.to_le_bytes());
        header[20..24].copy_from_slice(&(length | 0x2100_0000).to_le_bytes());
        header
    }

    #[test]
    fn borrowed_and_wrong_type_payloads_are_not_owned_save_strings() {
        let header = string_header(32);
        assert_eq!(string_layout(&header), Ok((0x1000, 32)));
        for (at, replacement) in [(0, 4u32), (16, 2), (20, 0x2000_0020), (20, 0x0100_0020)] {
            let mut changed = header;
            changed[at..at + 4].copy_from_slice(&replacement.to_le_bytes());
            assert_eq!(string_layout(&changed), Err(SaveError::Corrupt));
        }
        let mut changed = header;
        changed[8..16].fill(0);
        assert_eq!(string_layout(&changed), Err(SaveError::Corrupt));
    }

    #[test]
    fn complete_codec_envelope_fits_the_native_owned_string_width() {
        // The codec's maximum hex body alone already occupies 8 MiB. Its
        // prefix and the native NUL must fit too, without consuming body space.
        let width = MAX_PROPERTY_BYTES as u32 + 1;
        assert_eq!(
            string_layout(&string_header(width)),
            Ok((0x1000, width as usize))
        );
    }

    #[test]
    fn native_width_is_bounded_and_includes_the_terminating_byte() {
        assert!(string_layout(&string_header(MAX_VALUE as u32 + 1)).is_ok());
        assert_eq!(
            string_layout(&string_header(MAX_VALUE as u32 + 2)),
            Err(SaveError::Bounds)
        );
        for length in [0, 1] {
            assert_eq!(
                string_layout(&string_header(length)),
                Err(SaveError::Bounds)
            );
        }
        assert_eq!(
            NativeGameProperty::key("unrelated"),
            Err(SaveError::Identity)
        );
    }
}
