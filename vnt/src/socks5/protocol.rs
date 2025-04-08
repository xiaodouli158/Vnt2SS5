// SOCKS5 protocol constants

// SOCKS version
pub const SOCKS_VERSION: u8 = 5;

// Authentication methods
pub const NO_AUTH: u8 = 0;
pub const GSSAPI: u8 = 1;
pub const USERNAME_PASSWORD: u8 = 2;
pub const AUTH_METHOD_NOT_ACCEPTABLE: u8 = 0xFF;

// Commands
pub const CONNECT: u8 = 1;
pub const BIND: u8 = 2;
pub const UDP_ASSOCIATE: u8 = 3;

// Address types
pub const IPV4_ADDRESS: u8 = 1;
pub const DOMAIN_NAME: u8 = 3;
pub const IPV6_ADDRESS: u8 = 4;

// Reply codes
pub const SUCCEEDED: u8 = 0;
pub const GENERAL_FAILURE: u8 = 1;
pub const CONNECTION_NOT_ALLOWED: u8 = 2;
pub const NETWORK_UNREACHABLE: u8 = 3;
pub const HOST_UNREACHABLE: u8 = 4;
pub const CONNECTION_REFUSED: u8 = 5;
pub const TTL_EXPIRED: u8 = 6;
pub const COMMAND_NOT_SUPPORTED: u8 = 7;
pub const ADDRESS_TYPE_NOT_SUPPORTED: u8 = 8;
