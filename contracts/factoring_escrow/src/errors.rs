use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum EscrowError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    InvoiceNotVerified = 4,
    NotInvoiceOwner = 5,
    InvoiceAlreadyExpired = 6,
    EscrowNotFound = 7,
    EscrowAlreadySettled = 8,
    ArithmeticOverflow = 9,
    InvalidRateConfiguration = 10,
    DiscountExceedsFaceValue = 11,
    GracePeriodNotElapsed = 12,
    ContractPaused = 13,
    CurrencyMismatch = 14,
}
