//! JSON-RPC 2.0 error codes. Protocol values fixed by the JSON-RPC specification (the -327xx block) and by the
//! implementation-defined server range of the same specification (-32000 to -32099); they are not product numbers.
pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
pub const INTERNAL_ERROR: i64 = -32603;
/// The client has no grant, or its grant was revoked.
pub const NOT_AUTHORISED: i64 = -32001;
/// The owner pressed Refuse in the approval prompt.
pub const REFUSED: i64 = -32002;
/// The owner switched the server off.
pub const SERVER_OFF: i64 = -32003;
