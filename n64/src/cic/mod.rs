mod hle;

pub use hle::{CicHle, Fifo};

pub enum CIC {
    Nus6101,
    Nus6102,
    Nus6103,
    Nus6105,
    Nus6106,
    Nus7101,
    Nus7102,
    Nus7103,
    Nus7105,
    Nus7106,
    Nus8303,
    Nus8401,
    Nus5167,
    NusDDUS,
}
