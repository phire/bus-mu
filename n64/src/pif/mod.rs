mod hle;

pub use hle::{PifHle, PifIO};

pub enum Dir {
    Read,
    Write
}

pub enum Size {
    Size4,
    Size64
}
