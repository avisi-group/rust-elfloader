use {crate::ElfLoaderErr, xmas_elf::header::Machine};

pub mod aarch64;
pub mod arm;
pub mod riscv;
pub mod x86;
pub mod x86_64;

#[cfg(test)]
mod test;

#[derive(Debug)]
#[allow(non_camel_case_types)]
pub enum RelocationType {
    x86(x86::RelocationTypes),
    x86_64(x86_64::RelocationTypes),
    Arm(arm::RelocationTypes),
    AArch64(aarch64::RelocationTypes),
    RiscV(riscv::RelocationTypes),
}

impl RelocationType {
    /// Match an architecture and value to a Relocation type
    pub fn from(machine: Machine, type_num: u32) -> Result<RelocationType, ElfLoaderErr> {
        let typ = match machine {
            Machine::X86 => RelocationType::x86(x86::RelocationTypes::from(type_num)),
            Machine::X86_64 => RelocationType::x86_64(x86_64::RelocationTypes::from(type_num)),
            Machine::Arm => RelocationType::Arm(arm::RelocationTypes::from(type_num)),
            Machine::AArch64 => RelocationType::AArch64(aarch64::RelocationTypes::from(type_num)),
            Machine::RISC_V => RelocationType::RiscV(riscv::RelocationTypes::from(type_num)),
            _ => return Err(ElfLoaderErr::UnsupportedArchitecture),
        };
        Ok(typ)
    }
}
