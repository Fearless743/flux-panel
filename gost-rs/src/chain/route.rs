//! Route：单次拨号结果。
//!
//! 与 Go 版 `x/chain/route.go` 对应。

use crate::core::BoxedStream;

pub struct Route {
    /// 本跳选中的节点名。
    pub node: String,
    /// 节点地址。
    pub addr: String,
    /// 已建好的连接（已与目标完成握手/握手由 dialer/connector 处理）。
    pub conn: BoxedStream,
}