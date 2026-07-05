//! gost relay 二进制帧协议。
//!
//! 完整对应 `github.com/go-gost/relay@v0.5.0`。
//!
//! ## 帧格式
//!
//! Request：
//!
//! ```text
//! +-----+-------------+----+---+-----+----+
//! | VER |  CMD/FLAGS  | FEALEN | FEATURES |
//! +-----+-------------+----+---+-----+----+
//! |  1  |      1      |    2   |    VAR   |
//! ```
//!
//! Response：
//!
//! ```text
//! +-----+--------+----+---+-----+----+
//! | VER | STATUS | FEALEN | FEATURES |
//! +-----+--------+----+---+-----+----+
//! |  1  |    1   |    2   |    VAR   |
//! ```
//!
//! Feature：
//!
//! ```text
//! +------+----------+--------+
//! | TYPE |   LEN    |  DATA  |
//! +------+----------+--------+
//! |   1  |    2     |  VAR   |
//! ```

pub mod feature;
pub mod frame;

pub use feature::{
    AddrFeature, AddrType, Feature, FeatureType, NetworkFeature, NetworkID, TunnelFeature,
    UserAuthFeature,
};
pub use frame::{Request, Response};