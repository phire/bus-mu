

pub const COP0_REG_NAMES: [&'static str; 32] = [
    "Index",
    "Random",
    "EntryLo0",
    "EntryLo1",
    "Context",
    "PageMask",
    "Wired",
    "unk7",
    "BadVAddr",
    "Count",
    "EntryHi",
    "Compare",
    "Status",
    "Cause",
    "EPC", // Exception Program Counter
    "PRId", // Processor ID
    "Config",
    "LLAddr",
    "WatchLo",
    "WatchHi",
    "XContext",
    "unk21",
    "unk22",
    "unk23",
    "unk24",
    "unk25",
    "Parity Error",
    "Cache Error",
    "TagLo",
    "TagHi",
    "ErrorEPC",
    "unk31"
];
