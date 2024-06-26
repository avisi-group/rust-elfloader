#![no_std]
#![crate_name = "elfloader"]
#![crate_type = "lib"]

#[cfg(test)]
#[macro_use]
extern crate std;
#[cfg(test)]
extern crate env_logger;

// re-export all of `xmas_elf`
// `Flags` and `ProgramHeader` only because of their use in the `ElfLoader`
// trait
pub use xmas_elf::{
    self,
    program::{Flags, ProgramHeader},
};

mod binary;
pub use binary::ElfBinary;

pub mod arch;
pub use arch::RelocationType;

use {
    bitflags::bitflags,
    core::fmt,
    xmas_elf::{dynamic::*, header::Header},
};

/// Required alignment for zero-copy reads provided to xmas_elf by the
/// zero crate.
pub(crate) const ALIGNMENT: usize = core::mem::align_of::<Header>();

pub type PAddr = u64;
pub type VAddr = u64;

// Abstract relocation entries to be passed to the
// trait's relocate method. Library user can decide
// how to handle each relocation
#[allow(dead_code)]
pub struct RelocationEntry {
    pub rtype: RelocationType,
    pub offset: u64,
    pub index: u32,
    pub addend: Option<u64>,
}

#[derive(PartialEq, Clone, Debug)]
pub enum ElfLoaderErr {
    ElfParser { source: &'static str },
    OutOfMemory,
    UnalignedMemory,
    InvalidFlagsEntry,
    SymbolTableNotFound,
    UnsupportedElfFormat,
    UnsupportedElfVersion,
    UnsupportedEndianness,
    UnsupportedAbi,
    UnsupportedElfType,
    UnsupportedSectionData,
    UnsupportedArchitecture,
    UnsupportedRelocationEntry,
}

impl From<&'static str> for ElfLoaderErr {
    fn from(source: &'static str) -> Self {
        ElfLoaderErr::ElfParser { source }
    }
}

impl fmt::Display for ElfLoaderErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ElfLoaderErr::ElfParser { source } => write!(f, "Error in ELF parser: {}", source),
            ElfLoaderErr::OutOfMemory => write!(f, "Out of memory"),
            ElfLoaderErr::UnalignedMemory => write!(f, "Data must be aligned to {:?}", ALIGNMENT),
            ElfLoaderErr::InvalidFlagsEntry => {
                write!(f, "Failed to parse dynamic flags from entry bits")
            }
            ElfLoaderErr::SymbolTableNotFound => write!(f, "No symbol table in the ELF file"),
            ElfLoaderErr::UnsupportedElfFormat => write!(f, "ELF format not supported"),
            ElfLoaderErr::UnsupportedElfVersion => write!(f, "ELF version not supported"),
            ElfLoaderErr::UnsupportedEndianness => write!(f, "ELF endianness not supported"),
            ElfLoaderErr::UnsupportedAbi => write!(f, "ELF ABI not supported"),
            ElfLoaderErr::UnsupportedElfType => write!(f, "ELF type not supported"),
            ElfLoaderErr::UnsupportedSectionData => write!(f, "Can't handle this section data"),
            ElfLoaderErr::UnsupportedArchitecture => write!(f, "Unsupported Architecture"),
            ElfLoaderErr::UnsupportedRelocationEntry => {
                write!(f, "Can't handle relocation entry")
            }
        }
    }
}

bitflags! {
    #[derive(Default)]
    pub struct DynamicFlags1: u64 {
        const NOW = FLAG_1_NOW;
        const GLOBAL = FLAG_1_GLOBAL;
        const GROUP = FLAG_1_GROUP;
        const NODELETE = FLAG_1_NODELETE;
        const LOADFLTR = FLAG_1_LOADFLTR;
        const INITFIRST = FLAG_1_INITFIRST;
        const NOOPEN = FLAG_1_NOOPEN;
        const ORIGIN = FLAG_1_ORIGIN;
        const DIRECT = FLAG_1_DIRECT;
        const TRANS = FLAG_1_TRANS;
        const INTERPOSE = FLAG_1_INTERPOSE;
        const NODEFLIB = FLAG_1_NODEFLIB;
        const NODUMP = FLAG_1_NODUMP;
        const CONFALT = FLAG_1_CONFALT;
        const ENDFILTEE = FLAG_1_ENDFILTEE;
        const DISPRELDNE = FLAG_1_DISPRELDNE;
        const DISPRELPND = FLAG_1_DISPRELPND;
        const NODIRECT = FLAG_1_NODIRECT;
        const IGNMULDEF = FLAG_1_IGNMULDEF;
        const NOKSYMS = FLAG_1_NOKSYMS;
        const NOHDR = FLAG_1_NOHDR;
        const EDITED = FLAG_1_EDITED;
        const NORELOC = FLAG_1_NORELOC;
        const SYMINTPOSE = FLAG_1_SYMINTPOSE;
        const GLOBAUDIT = FLAG_1_GLOBAUDIT;
        const SINGLETON = FLAG_1_SINGLETON;
        const STUB = FLAG_1_STUB;
        const PIE = FLAG_1_PIE;
    }
}

/// Information parse from the .dynamic section
pub struct DynamicInfo {
    pub flags1: DynamicFlags1,
    pub rela: u64,
    pub rela_size: u64,
}

/// Implement this trait for customized ELF loading.
///
/// The flow of ElfBinary is that it first calls `allocate` for all regions
/// that need to be allocated (i.e., the LOAD program headers of the ELF
/// binary), then `load` will be called to fill the allocated regions, and
/// finally `relocate` is called for every entry in the RELA table.
pub trait ElfLoader {
    /// Allocates a virtual region specified by `header`.
    fn allocate(&mut self, header: ProgramHeader) -> Result<(), ElfLoaderErr>;

    /// Copies `region` into memory starting at `base`.
    /// The caller makes sure that there was an `allocate` call previously
    /// to initialize the region.
    fn load(&mut self, flags: Flags, base: VAddr, region: &[u8]) -> Result<(), ElfLoaderErr>;

    /// Request for the client to relocate the given `entry`
    /// within the loaded ELF file.
    fn relocate(&mut self, entry: RelocationEntry) -> Result<(), ElfLoaderErr>;

    /// Inform client about where the initial TLS data is located.
    fn tls(
        &mut self,
        _tdata_start: VAddr,
        _tdata_length: u64,
        _total_size: u64,
        _align: u64,
    ) -> Result<(), ElfLoaderErr> {
        Ok(())
    }

    /// In case there is a `.data.rel.ro` section we instruct the loader
    /// to change the passed offset to read-only (this is called after
    /// the relocate calls are completed).
    ///
    /// Note: The default implementation is a no-op since this is
    /// not strictly necessary to implement.
    fn make_readonly(&mut self, _base: VAddr, _size: usize) -> Result<(), ElfLoaderErr> {
        Ok(())
    }
}

/// Utility function to verify alignment.
///
/// Note: this may be stabilized in the future as:
///
/// [core::ptr::is_aligned_to](https://doc.rust-lang.org/core/primitive.pointer.html#method.is_aligned_to)
pub(crate) fn is_aligned_to(ptr: usize, align: usize) -> bool {
    ptr & (align - 1) == 0
}

#[cfg(doctest)]
mod test_readme {
    macro_rules! external_doc_test {
        ($x:expr) => {
            #[doc = $x]
            extern "C" {}
        };
    }

    external_doc_test!(include_str!("../README.md"));
}
