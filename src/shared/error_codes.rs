//! 错误码常量定义

// 4xx 客户端错误
pub const BAD_REQUEST: i32 = 40000;
pub const INVALID_INPUT: i32 = 40001;

pub const UNAUTHORIZED: i32 = 40100;
pub const INVALID_CREDENTIALS: i32 = 40101;
pub const TOKEN_EXPIRED: i32 = 40102;

pub const FORBIDDEN: i32 = 40300;

pub const NOT_FOUND: i32 = 40400;
pub const RESOURCE_NOT_FOUND: i32 = 40401;

pub const CONFLICT: i32 = 40900;
pub const RESOURCE_ALREADY_EXISTS: i32 = 40901;

pub const TOO_MANY_REQUESTS: i32 = 42900;

// 5xx 服务器错误
pub const INTERNAL_ERROR: i32 = 50000;
pub const DATABASE_ERROR: i32 = 50001;
pub const CONFIG_ERROR: i32 = 50002;
