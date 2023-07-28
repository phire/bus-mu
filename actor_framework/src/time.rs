
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Time {
    // TODO: allow lazy times
    pub(crate) cycles: u64
}

impl Time {
    pub fn max() -> Self {
        Time {
            cycles: u64::MAX
        }
    }

    pub fn is_resolved(&self) -> bool {
        // TODO: allow lazy times
        self.cycles != u64::MAX && self.cycles != 0
    }

    pub fn add(&self, other: u64) -> Self {
        Time {
            cycles: self.cycles + other
        }
    }
}

impl Default for Time {
    fn default() -> Self {
        Time {
            cycles: 0,
        }
    }
}

impl From<Time> for u64 {
    fn from(value: Time) -> Self {
        value.cycles
    }
}

impl From<u64> for Time {
    fn from(cycles: u64) -> Self {
        Time {
            cycles
        }
    }
}