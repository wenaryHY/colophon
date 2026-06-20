//! DNS 解析器抽象
//!
//! 将 DNS 解析与具体实现解耦，使 dispatcher 的 SSRF 防护逻辑可脱离真实网络测试。
//! 生产环境注入 [`TokioDnsResolver`]，测试注入 [`MockResolver`]。

use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;

/// DNS 解析器抽象，解耦 dispatcher 与 tokio::net::lookup_host。
/// 生产环境用 [`TokioDnsResolver`]，测试用 MockResolver。
pub trait DnsResolver: Send + Sync {
    fn lookup_host(
        &self,
        host: &str,
        port: u16,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<SocketAddr>, std::io::Error>> + Send + '_>>;
}

/// 生产环境实现：委托给 tokio::net::lookup_host
pub struct TokioDnsResolver;

impl DnsResolver for TokioDnsResolver {
    fn lookup_host(
        &self,
        host: &str,
        port: u16,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<SocketAddr>, std::io::Error>> + Send + '_>> {
        let addr_str = format!("{}:{}", host, port);
        Box::pin(async move {
            tokio::net::lookup_host(addr_str)
                .await
                .map(|iter| iter.collect())
        })
    }
}
