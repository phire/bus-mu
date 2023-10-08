use core::fmt;
use std::ops::BitAnd;

#[derive(Copy, Clone)]
pub struct ByteMask8 {
    mask: u64,
}

impl ByteMask8 {
    #[inline(always)]
    pub fn new<W, A>(width: W, alignment: A) -> Self
    where u32: From<W>, u32: From<A> {
        let mask = (!0u64).wrapping_shl(64 - u32::from(width) * 8) >> u32::from(alignment) * 8;
        ByteMask8 { mask }
    }

    #[inline(always)]
    pub fn apply(&self, data: u64) -> u64 {
        data & self.mask
    }

    // #[inline(always)]
    // pub fn clear(&self, data: &mut u64) {
    //     *data &= !self.mask;
    // }

    #[inline(always)]
    pub fn masked_insert(&self, dest: &mut u64, value: u64) {
        *dest = (*dest & !self.mask) | (value & self.mask);
    }

    #[inline(always)]
    pub fn value(&self) -> u64 {
        self.mask
    }

    pub fn size(&self) -> u32 {
        self.mask.count_ones()
    }
}

impl BitAnd for ByteMask8 {
    type Output = Self;

    #[inline(always)]
    fn bitand(self, rhs: Self) -> Self::Output {
        ByteMask8 {
            mask: self.mask & rhs.mask,
        }
    }
}

impl Default for ByteMask8 {
    #[inline(always)]
    fn default() -> Self {
        ByteMask8 { mask: !0 }
    }
}

impl fmt::Debug for ByteMask8 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ByteMask8({:016x})", self.mask)
    }
}
