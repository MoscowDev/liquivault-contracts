use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum PoolError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    InvalidAmount = 4,
    InsufficientShares = 5,
    InsufficientPoolLiquidity = 6,
    ArithmeticOverflow = 7,
    ZeroSharesMinted = 8,
    ZeroAssetsReturned = 9,
    InvalidReserveFactor = 10,
    PoolPaused = 11,
    RepaymentExceedsDeployed = 12,
    TrancheWipedOut = 13,
    JuniorCoverageTooLow = 14,
    InvalidJuniorYieldBps = 15,
}

