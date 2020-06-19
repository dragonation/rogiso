mod property_trap;
mod slot_trap;
mod trap_info;

pub use property_trap::PropertyTrap;
pub use property_trap::ProtectedPropertyTrap;

pub use property_trap::FieldPropertyTrap;

pub use slot_trap::SlotTrap;
pub use slot_trap::SlotTrapResult;
pub use slot_trap::ProtectedSlotTrap;

pub use trap_info::TrapInfo;
