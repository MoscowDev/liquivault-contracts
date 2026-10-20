use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum RegistryError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    InvoiceNotFound = 4,
    InvalidFaceValue = 5,
    InvalidMaturityTimestamp = 6,
    InvalidStatusTransition = 7,
    InvoiceAlreadyFactored = 8,
    ArithmeticOverflow = 9,
    SameParties = 10,
    ContractPaused = 11,
}
