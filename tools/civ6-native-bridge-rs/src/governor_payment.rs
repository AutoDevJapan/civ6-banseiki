//! Copies the pinned Core Governor's own promotion flags during a natural
//! CanAssign invocation. This is a read-only snapshot, never a persisted permit.
//! No Core getter is called and no game memory is changed.

pub const PROMOTION_LIMIT: usize = 4096;
pub const PROMOTION_WORDS: usize = PROMOTION_LIMIT / 64;
const GOVERNOR_LIMIT: usize = 256;

pub trait CurrentMemory {
    fn copy<const N: usize>(&self, address: u64) -> Option<[u8; N]>;
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadFailure {
    Unreadable = 1,
    InvalidLayout = 2,
    IdentityMismatch = 3,
    NotMember = 4,
    Changed = 5,
    InvalidFlag = 6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PromotionSnapshot {
    pub owner: u32,
    pub governor_index: u32,
    pub count: u32,
    pub owned: [u64; PROMOTION_WORDS],
}

impl PromotionSnapshot {
    pub fn has(&self, index: u32) -> Option<bool> {
        (index < self.count && index < PROMOTION_LIMIT as u32)
            .then(|| self.owned[index as usize / 64] & (1 << (index % 64)) != 0)
    }
}

fn copy<const N: usize>(memory: &impl CurrentMemory, address: u64) -> Result<[u8; N], ReadFailure> {
    if address == 0 || address.checked_add(N as u64).is_none() {
        return Err(ReadFailure::Unreadable);
    }
    memory.copy(address).ok_or(ReadFailure::Unreadable)
}

fn field<const N: usize>(
    memory: &impl CurrentMemory,
    base: u64,
    offset: u64,
) -> Result<[u8; N], ReadFailure> {
    copy(
        memory,
        base.checked_add(offset).ok_or(ReadFailure::Unreadable)?,
    )
}

fn member(memory: &impl CurrentMemory, head: u64, governor: u64) -> Result<(), ReadFailure> {
    let mut node = head;
    let mut visited = [0u64; GOVERNOR_LIMIT];
    for step in 0..GOVERNOR_LIMIT {
        if node == 0 {
            return Err(ReadFailure::NotMember);
        }
        // A repeated node is rejected by the finite traversal even if a corrupt
        // chain would otherwise eventually point at the requested Governor.
        if visited[..step].contains(&node) {
            return Err(ReadFailure::InvalidLayout);
        }
        visited[step] = node;
        let entry = copy::<24>(memory, node)?;
        if u64::from_le_bytes(entry[..8].try_into().unwrap()) == governor {
            return Ok(());
        }
        let next = u64::from_le_bytes(entry[16..24].try_into().unwrap());
        if next == node {
            return Err(ReadFailure::InvalidLayout);
        }
        node = next;
    }
    Err(ReadFailure::InvalidLayout)
}

fn flags(
    memory: &impl CurrentMemory,
    address: u64,
    count: usize,
) -> Result<[u8; PROMOTION_LIMIT], ReadFailure> {
    let mut bytes = [0; PROMOTION_LIMIT];
    let complete = count / 8 * 8;
    for offset in (0..complete).step_by(8) {
        bytes[offset..offset + 8].copy_from_slice(&field::<8>(memory, address, offset as u64)?);
    }
    for offset in complete..count {
        bytes[offset] = field::<1>(memory, address, offset as u64)?[0];
    }
    Ok(bytes)
}

/// `governors` and `governor` must be the two current CanAssign arguments.
/// Result contains scalar identities and copied flags, never a reusable pointer.
pub fn read_current(
    memory: &impl CurrentMemory,
    governors: u64,
    governor: u64,
) -> Result<PromotionSnapshot, ReadFailure> {
    let actor = field::<8>(memory, governors, 0x70)?;
    let player = u64::from_le_bytes(actor);
    let owner_bytes = field::<4>(memory, player, 0xd8)?;
    let owner = u32::from_le_bytes(owner_bytes);
    if owner >= 64 {
        return Err(ReadFailure::IdentityMismatch);
    }
    let head_bytes = field::<8>(memory, governors, 0x58)?;
    let head = u64::from_le_bytes(head_bytes);
    member(memory, head, governor)?;
    let identity = copy::<8>(memory, governor)?;
    let actual_owner = u32::from_le_bytes(identity[..4].try_into().unwrap());
    let index = u32::from_le_bytes(identity[4..].try_into().unwrap());
    if actual_owner != owner || index >= GOVERNOR_LIMIT as u32 {
        return Err(ReadFailure::IdentityMismatch);
    }
    // Pinned 259980 reads flags at +68. 2598f0 counts nonzero bytes up to
    // length +78. 259990 allocates capacity +70, then publishes length +78.
    let header = field::<20>(memory, governor, 0x68)?;
    let flag_address = u64::from_le_bytes(header[..8].try_into().unwrap());
    let capacity = u32::from_le_bytes(header[8..12].try_into().unwrap());
    let count = u32::from_le_bytes(header[16..20].try_into().unwrap());
    if count == 0 || count > capacity || capacity > PROMOTION_LIMIT as u32 || flag_address == 0 {
        return Err(ReadFailure::InvalidLayout);
    }
    let bytes = flags(memory, flag_address, count as usize)?;
    let mut owned = [0u64; PROMOTION_WORDS];
    for index in 0..count as usize {
        let value = bytes[index];
        if value > 1 {
            return Err(ReadFailure::InvalidFlag);
        }
        if value == 1 {
            owned[index / 64] |= 1 << (index % 64);
        }
    }
    // Repeat all selected bytes and identities. This detects torn observations;
    // it does not assert global quiescence or turn a snapshot into authority.
    if flags(memory, flag_address, count as usize)? != bytes {
        return Err(ReadFailure::Changed);
    }
    if field::<8>(memory, governors, 0x70)? != actor
        || field::<4>(memory, player, 0xd8)? != owner_bytes
        || field::<8>(memory, governors, 0x58)? != head_bytes
        || copy::<8>(memory, governor)? != identity
        || field::<20>(memory, governor, 0x68)? != header
    {
        return Err(ReadFailure::Changed);
    }
    member(memory, head, governor)?;
    Ok(PromotionSnapshot {
        owner,
        governor_index: index,
        count,
        owned,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    struct Memory {
        bytes: [u8; 32768],
        mutate: Cell<Option<u64>>,
        reads: Cell<usize>,
    }
    impl CurrentMemory for Memory {
        fn copy<const N: usize>(&self, address: u64) -> Option<[u8; N]> {
            let count = self.reads.get();
            self.reads.set(count + 1);
            let start = usize::try_from(address).ok()?;
            let mut value: [u8; N] = self
                .bytes
                .get(start..start.checked_add(N)?)?
                .try_into()
                .ok()?;
            if self.mutate.get() == Some(address) && count > 12 {
                value[0] ^= 1;
            }
            Some(value)
        }
    }
    fn fixture() -> Memory {
        let mut m = Memory {
            bytes: [0; 32768],
            mutate: Cell::new(None),
            reads: Cell::new(0),
        };
        put(&mut m, 0x1070, &0x2000u64.to_le_bytes());
        put(&mut m, 0x20d8, &7u32.to_le_bytes());
        put(&mut m, 0x1058, &0x3000u64.to_le_bytes());
        put(&mut m, 0x3000, &0x3800u64.to_le_bytes());
        put(&mut m, 0x3800, &7u32.to_le_bytes());
        put(&mut m, 0x3804, &22u32.to_le_bytes());
        put(&mut m, 0x3868, &0x4000u64.to_le_bytes());
        put(&mut m, 0x3870, &4096u32.to_le_bytes());
        put(&mut m, 0x3878, &4096u32.to_le_bytes());
        for i in [0, 63, 64, 1000, 4095] {
            m.bytes[0x4000 + i] = 1;
        }
        m
    }
    fn put(m: &mut Memory, address: usize, bytes: &[u8]) {
        m.bytes[address..address + bytes.len()].copy_from_slice(bytes);
    }

    #[test]
    fn copies_this_governors_own_flags_at_word_and_capacity_boundaries() {
        let m = fixture();
        let before = m.bytes;
        let s = read_current(&m, 0x1000, 0x3800).unwrap();
        assert_eq!((s.owner, s.governor_index, s.count), (7, 22, 4096));
        for i in [0, 63, 64, 1000, 4095] {
            assert_eq!(s.has(i), Some(true));
        }
        for i in [1, 62, 65, 999, 4094] {
            assert_eq!(s.has(i), Some(false));
        }
        assert_eq!(s.has(4096), None);
        assert_eq!(m.bytes, before);
    }
    #[test]
    fn rejects_wrong_owner_nonmember_and_invalid_lengths_without_getters() {
        let mut m = fixture();
        put(&mut m, 0x3800, &8u32.to_le_bytes());
        assert_eq!(
            read_current(&m, 0x1000, 0x3800),
            Err(ReadFailure::IdentityMismatch)
        );
        let m = fixture();
        assert_eq!(
            read_current(&m, 0x1000, 0x3900),
            Err(ReadFailure::NotMember)
        );
        for (offset, value) in [(0x3870, 4097u32), (0x3878, 4097), (0x3878, 0)] {
            let mut m = fixture();
            put(&mut m, offset, &value.to_le_bytes());
            assert_eq!(
                read_current(&m, 0x1000, 0x3800),
                Err(ReadFailure::InvalidLayout)
            );
        }
    }
    #[test]
    fn rejects_unreadable_relocated_and_non_boolean_flags() {
        let mut m = fixture();
        m.bytes[0x4001] = 2;
        assert_eq!(
            read_current(&m, 0x1000, 0x3800),
            Err(ReadFailure::InvalidFlag)
        );
        let mut m = fixture();
        put(&mut m, 0x3868, &0xfffffffffffffff0u64.to_le_bytes());
        assert_eq!(
            read_current(&m, 0x1000, 0x3800),
            Err(ReadFailure::Unreadable)
        );
        let m = fixture();
        m.mutate.set(Some(0x4000));
        assert_eq!(read_current(&m, 0x1000, 0x3800), Err(ReadFailure::Changed));
        let m = fixture();
        m.mutate.set(Some(0x3868));
        assert_eq!(read_current(&m, 0x1000, 0x3800), Err(ReadFailure::Changed));
    }
    #[test]
    fn rejects_cyclic_membership_with_a_finite_bound() {
        let mut m = fixture();
        put(&mut m, 0x3000, &0x3900u64.to_le_bytes());
        put(&mut m, 0x3010, &0x3020u64.to_le_bytes());
        put(&mut m, 0x3030, &0x3000u64.to_le_bytes());
        assert_eq!(
            read_current(&m, 0x1000, 0x3800),
            Err(ReadFailure::InvalidLayout)
        );
        assert!(m.reads.get() < 270);
    }
}
