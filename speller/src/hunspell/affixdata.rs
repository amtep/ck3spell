/// Represents the format of the flags after words in the dictionary file.
pub enum FlagMode {
    /// Single-character flags
    CharFlags,
    /// Two-character flags
    DoubleCharFlags,
    /// Flags are comma-separated ASCII integers
    NumericFlags,
    /// Flags are Unicode codepoints in UTF-8 format
    Utf8Flags,
}

pub struct AffixData {
    pub flag_mode: FlagMode,
}

impl AffixData {
    pub fn new() -> Self {
        AffixData {
            flag_mode: FlagMode::CharFlags,
        }
    }
}
