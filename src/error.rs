//! Error handling utils. See ["failure" crate documentation](https://docs.rs/failure/) for details
//!
//! The Tarantool error handling works most like libc's errno. All API calls
//! return -1 or `NULL` in the event of error. An internal pointer to
//! `box_error_t` type is set by API functions to indicate what went wrong.
//! This value is only significant if API call failed (returned -1 or `NULL`).
//!
//! Successful function can also touch the last error in some
//! cases. You don't have to clear the last error before calling
//! API functions. The returned object is valid only until next
//! call to **any** API function.
//!
//! You must set the last error using `set_error()` in your stored C
//! procedures if you want to return a custom error message.
//! You can re-throw the last API error to IPROTO client by keeping
//! the current value and returning -1 to Tarantool from your
//! stored procedure.

use std::ffi::{CStr, CString};
use std::os::raw::c_int;
use std::{fmt, io};

use failure::_core::fmt::{Display, Formatter};
use num_traits::{FromPrimitive, ToPrimitive};
use rmp::decode::ValueReadError;
use rmp::encode::ValueWriteError;
use rmp_serde::decode::Error as DecodeError;
use rmp_serde::encode::Error as EncodeError;

/// Represents all error cases for all routines of crate (including Tarantool errors)
#[derive(Debug, Fail)]
pub enum Error {
    #[fail(display = "Tarantool error: {}", _0)]
    Tarantool(TarantoolError),

    #[fail(display = "IO error: {}", _0)]
    IO(io::Error),

    #[fail(display = "Failed to encode tuple: {}", _0)]
    Encode(EncodeError),

    #[fail(display = "Failed to decode tuple: {}", _0)]
    Decode(DecodeError),

    #[fail(display = "Value read error: {}", _0)]
    ValueRead(ValueReadError),

    #[fail(display = "Value write error: {}", _0)]
    ValueWrite(ValueWriteError),

    #[fail(display = "Transaction issue: {}", _0)]
    Transaction(TransactionError),
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Error::IO(error)
    }
}

impl From<EncodeError> for Error {
    fn from(error: EncodeError) -> Self {
        Error::Encode(error)
    }
}

impl From<DecodeError> for Error {
    fn from(error: DecodeError) -> Self {
        Error::Decode(error)
    }
}

impl From<ValueReadError> for Error {
    fn from(error: ValueReadError) -> Self {
        Error::ValueRead(error)
    }
}

impl From<ValueWriteError> for Error {
    fn from(error: ValueWriteError) -> Self {
        Error::ValueWrite(error)
    }
}

/// Transaction-related error cases
#[derive(Debug, Fail)]
pub enum TransactionError {
    #[fail(display = "Transaction has already been started")]
    AlreadyStarted,

    #[fail(display = "Failed to commit")]
    FailedToCommit,

    #[fail(display = "Failed to rollback")]
    FailedToRollback,
}

impl From<TransactionError> for Error {
    fn from(error: TransactionError) -> Self {
        Error::Transaction(error)
    }
}

/// Settable by Tarantool error type
#[derive(Derivative)]
#[derivative(Debug)]
pub struct TarantoolError {
    code: TarantoolErrorCode,
    message: String,
    #[derivative(Debug = "ignore")]
    error_ptr: Box<ffi::BoxError>,
}

impl TarantoolError {
    /// Tries to get the information about the last API call error. If error was not set
    /// returns `Ok(())`
    pub fn maybe_last() -> Result<(), Self> {
        let error_ptr = unsafe { ffi::box_error_last() };
        if error_ptr.is_null() {
            return Ok(());
        }

        let code = unsafe { ffi::box_error_code(error_ptr) };
        let code = match TarantoolErrorCode::from_u32(code) {
            Some(code) => code,
            None => TarantoolErrorCode::Unknown,
        };

        let message = unsafe { CStr::from_ptr(ffi::box_error_message(error_ptr)) };
        let message = message.to_string_lossy().into_owned();

        Err(TarantoolError {
            code,
            message,
            error_ptr: unsafe { Box::from_raw(error_ptr) },
        })
    }

    /// Get the information about the last API call error.
    pub fn last() -> Self {
        TarantoolError::maybe_last().err().unwrap()
    }

    /// Return IPROTO error code
    pub fn error_code(&self) -> TarantoolErrorCode {
        self.code.clone()
    }

    /// Return the error type, e.g. "ClientError", "SocketError", etc.
    pub fn error_type(&self) -> String {
        let result = unsafe { ffi::box_error_type(&*self.error_ptr) };
        unsafe { CStr::from_ptr(result) }
            .to_string_lossy()
            .to_string()
    }
}

impl Display for TarantoolError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)
    }
}

impl From<TarantoolError> for Error {
    fn from(error: TarantoolError) -> Self {
        Error::Tarantool(error)
    }
}

/// Codes of Tarantool errors
#[repr(u32)]
#[derive(Debug, Clone, PartialEq, ToPrimitive, FromPrimitive)]
pub enum TarantoolErrorCode {
    Unknown = 0,
    IllegalParams = 1,
    MemoryIssue = 2,
    TupleFound = 3,
    TupleNotFound = 4,
    Unsupported = 5,
    NonMaster = 6,
    Readonly = 7,
    Injection = 8,
    CreateSpace = 9,
    SpaceExists = 10,
    DropSpace = 11,
    AlterSpace = 12,
    IndexType = 13,
    ModifyIndex = 14,
    LastDrop = 15,
    TupleFormatLimit = 16,
    DropPrimaryKey = 17,
    KeyPartType = 18,
    ExactMatch = 19,
    InvalidMsgpack = 20,
    ProcRet = 21,
    TupleNotArray = 22,
    FieldType = 23,
    IndexPartTypeMismatch = 24,
    Splice = 25,
    UpdateArgType = 26,
    FormatMismatchIndexPart = 27,
    UnknownUpdateOp = 28,
    UpdateField = 29,
    FunctionTxActive = 30,
    KeyPartCount = 31,
    ProcLua = 32,
    NoSuchProc = 33,
    NoSuchTrigger = 34,
    NoSuchIndexID = 35,
    NoSuchSpace = 36,
    NoSuchFieldNo = 37,
    ExactFieldCount = 38,
    FieldMissing = 39,
    WalIo = 40,
    MoreThanOneTuple = 41,
    AccessDenied = 42,
    CreateUser = 43,
    DropUser = 44,
    NoSuchUser = 45,
    UserExists = 46,
    PasswordMismatch = 47,
    UnknownRequestType = 48,
    UnknownSchemaObject = 49,
    CreateFunction = 50,
    NoSuchFunction = 51,
    FunctionExists = 52,
    BeforeReplaceRet = 53,
    MultistatementTransaction = 54,
    TriggerExists = 55,
    UserMax = 56,
    NoSuchEngine = 57,
    ReloadCfg = 58,
    Cfg = 59,
    SavepointEmptyTx = 60,
    NoSuchSavepoint = 61,
    UnknownReplica = 62,
    ReplicasetUuidMismatch = 63,
    InvalidUuid = 64,
    ReplicasetUuidIsRo = 65,
    InstanceUuidMismatch = 66,
    ReplicaIDIsReserved = 67,
    InvalidOrder = 68,
    MissingRequestField = 69,
    Identifier = 70,
    DropFunction = 71,
    IteratorType = 72,
    ReplicaMax = 73,
    InvalidXlog = 74,
    InvalidXlogName = 75,
    InvalidXlogOrder = 76,
    NoConnection = 77,
    Timeout = 78,
    ActiveTransaction = 79,
    CursorNoTransaction = 80,
    CrossEngineTransaction = 81,
    NoSuchRole = 82,
    RoleExists = 83,
    CreateRole = 84,
    IndexExists = 85,
    SessionClosed = 86,
    RoleLoop = 87,
    Grant = 88,
    PrivGranted = 89,
    RoleGranted = 90,
    PrivNotGranted = 91,
    RoleNotGranted = 92,
    MissingSnapshot = 93,
    CantUpdatePrimaryKey = 94,
    UpdateIntegerOverflow = 95,
    GuestUserPassword = 96,
    TransactionConflict = 97,
    UnsupportedPriv = 98,
    LoadFunction = 99,
    FunctionLanguage = 100,
    RtreeRect = 101,
    ProcC = 102,
    UnknownRtreeIndexDistanceType = 103,
    Protocol = 104,
    UpsertUniqueSecondaryKey = 105,
    WrongIndexRecord = 106,
    WrongIndexParts = 107,
    WrongIndexOptions = 108,
    WrongSchemaVersion = 109,
    MemtxMaxTupleSize = 110,
    WrongSpaceOptions = 111,
    UnsupportedIndexFeature = 112,
    ViewIsRo = 113,
    NoTransaction = 114,
    System = 115,
    Loading = 116,
    ConnectionToSelf = 117,
    KeyPartIsTooLong = 118,
    Compression = 119,
    CheckpointInProgress = 120,
    SubStmtMax = 121,
    CommitInSubStmt = 122,
    RollbackInSubStmt = 123,
    Decompression = 124,
    InvalidXlogType = 125,
    AlreadyRunning = 126,
    IndexFieldCountLimit = 127,
    LocalInstanceIDIsReadOnly = 128,
    BackupInProgress = 129,
    ReadViewAborted = 130,
    InvalidIndexFile = 131,
    InvalidRunFile = 132,
    InvalidVylogFile = 133,
    CheckpointRollback = 134,
    VyQuotaTimeout = 135,
    PartialKey = 136,
    TruncateSystemSpace = 137,
    LoadModule = 138,
    VinylMaxTupleSize = 139,
    WrongDdVersion = 140,
    WrongSpaceFormat = 141,
    CreateSequence = 142,
    AlterSequence = 143,
    DropSequence = 144,
    NoSuchSequence = 145,
    SequenceExists = 146,
    SequenceOverflow = 147,
    NoSuchIndexName = 148,
    SpaceFieldIsDuplicate = 149,
    CantCreateCollation = 150,
    WrongCollationOptions = 151,
    NullablePrimary = 152,
    NoSuchFieldName = 153,
    TransactionYield = 154,
    NoSuchGroup = 155,
    SqlBindValue = 156,
    SqlBindType = 157,
    SqlBindParameterMax = 158,
    SqlExecute = 159,
    Unused = 160,
    SqlBindNotFound = 161,
    ActionMismatch = 162,
    ViewMissingSql = 163,
    ForeignKeyConstraint = 164,
    NoSuchModule = 165,
    NoSuchCollation = 166,
    CreateFkConstraint = 167,
    DropFkConstraint = 168,
    NoSuchConstraint = 169,
    ConstraintExists = 170,
    SqlTypeMismatch = 171,
    RowidOverflow = 172,
    DropCollation = 173,
    IllegalCollationMix = 174,
    SqlNoSuchPragma = 175,
    SqlCantResolveField = 176,
    IndexExistsInSpace = 177,
    InconsistentTypes = 178,
    SqlSyntax = 179,
    SqlStackOverflow = 180,
    SqlSelectWildcard = 181,
    SqlStatementEmpty = 182,
    SqlKeywordIsReserved = 183,
    SqlUnrecognizedSyntax = 184,
    SqlUnknownToken = 185,
    SqlParserGeneric = 186,
    SqlAnalyzeArgument = 187,
    SqlColumnCountMax = 188,
    HexLiteralMax = 189,
    IntLiteralMax = 190,
    SqlParserLimit = 191,
    IndexDefUnsupported = 192,
    CkDefUnsupported = 193,
    MultikeyIndexMismatch = 194,
    CreateCkConstraint = 195,
    CkConstraintFailed = 196,
    SqlColumnCount = 197,
    FuncIndexFunc = 198,
    FuncIndexFormat = 199,
    FuncIndexParts = 200,
    BootstrapReadonly = 201,
}

/// Clear the last error.
pub fn clear_error() {
    unsafe { ffi::box_error_clear() }
}

/// Set the last error.
pub fn set_error(file: &str, line: u32, code: &TarantoolErrorCode, msg: &str) -> c_int {
    unsafe {
        ffi::box_error_set(
            CString::new(file).unwrap().as_ptr(),
            line,
            code.to_u32().unwrap(),
            CString::new(msg).unwrap().as_ptr(),
        )
    }
}

mod ffi {
    use std::os::raw::{c_char, c_int, c_uint};

    #[repr(C)]
    pub struct BoxError {
        _unused: [u8; 0],
    }

    extern "C" {
        pub fn box_error_code(error: *const BoxError) -> u32;
        pub fn box_error_message(error: *const BoxError) -> *const c_char;
        pub fn box_error_last() -> *mut BoxError;
        pub fn box_error_type(error: *const BoxError) -> *const c_char;
        pub fn box_error_clear();
        pub fn box_error_set(
            file: *const c_char,
            line: c_uint,
            code: u32,
            format: *const c_char,
            ...
        ) -> c_int;
    }
}
