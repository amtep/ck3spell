#![allow(non_upper_case_globals)]

use bitflags::bitflags;

bitflags! {
    #[derive(Default)]
    pub struct WordFlags: u16 {
        /// This word must not be accepted as good.
        const Forbidden = 0x0001;
        /// This word may appear at the beginning of compound words.
        const CompoundBegin = 0x0002;
        /// This word may appear as a middle word in compound words.
        const CompoundMiddle = 0x0004;
        /// This word may appear at the end of compound words.
        const CompoundEnd = 0x0008;
        /// This word may have affixes even inside a compound word.
        const CompoundPermit = 0x0010;
        /// This word can only appear as part of compound words.
        const OnlyInCompound = 0x0020;
        /// This word must not be suggested as a correction.
        const NoSuggest = 0x0040;
        /// A continuation flag, for PFX and SFX that must surround a word.
        const Circumfix = 0x0080;
        /// This word is not valid without an affix.
        const NeedAffix = 0x0100;
        /// This word should not have its case changed.
        const KeepCase = 0x0200;
        /// This word may appear in compounds.
        /// (predates the CompoundBegin, Middle, End flags)
        const CompoundFlag = 0x0400;
        /// This is a very rare word that is likely a spelling error
        const Warn = 0x0800;
    }
}
