
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Time {
    // TODO: allow lazy times
    pub(crate) cycles: u64
}

impl Default for Time {
    fn default() -> Self {
        Time {
            cycles: 0,
        }
    }
}
