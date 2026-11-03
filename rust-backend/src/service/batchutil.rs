use futures_util::future::BoxFuture;
use futures_util::stream::StreamExt;
use futures_util::Future;

/// 批量操作并发度：受节点 WS 往返（最长 10s）主导，IO 等待期间不占 DB 连接，
/// 提高并发可显著缩短批量删除/暂停/恢复/迁移的总耗时
pub const BATCH_CONCURRENCY: usize = 16;

/// 保序去重（批量接口按唯一 ID 处理，避免重复下发/重复计数）
pub fn dedup_int64s(ids: &[i64]) -> Vec<i64> {
    let mut seen = std::collections::HashSet::with_capacity(ids.len());
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        if seen.insert(*id) {
            out.push(*id);
        }
    }
    out
}

/// 对 0..n-1 限并发执行 fn，返回按下标顺序收集的非空错误文本。
/// fn 返回 "" 表示成功；只读共享数据可安全并发访问。
pub async fn parallel_each<F, Fut>(n: usize, workers: usize, f: F) -> Vec<String>
where
    F: Fn(usize) -> Fut,
    Fut: Future<Output = String>,
{
    parallel_each_indexed(n, workers, f)
        .await
        .into_iter()
        .map(|(_, msg)| msg)
        .collect()
}

/// 与 parallel_each 相同，但保留失败项的下标，且按下标升序返回。
pub async fn parallel_each_indexed<F, Fut>(
    n: usize,
    workers: usize,
    f: F,
) -> Vec<(usize, String)>
where
    F: Fn(usize) -> Fut,
    Fut: Future<Output = String>,
{
    if n == 0 {
        return Vec::new();
    }
    let workers = if workers == 0 || workers > n { n } else { workers };

    let mut out: Vec<(usize, String)> = futures_util::stream::iter(0..n)
        .map(|i| {
            let fut = f(i);
            async move {
                let msg = fut.await;
                (i, msg)
            }
        })
        .buffer_unordered(workers)
        .filter_map(|(i, msg)| async move {
            if msg.is_empty() {
                None
            } else {
                Some((i, msg))
            }
        })
        .collect()
        .await;
    out.sort_by_key(|(i, _)| *i);
    out
}

/// 一次面向单个节点的下发动作。
/// 组内（同 nodeID）严格按加入顺序串行，组间（不同 nodeID）限并发——
/// 既保证同节点上“先建 chain、再建引用它的 service”的顺序，又让多节点下发重叠 IO 等待。
pub struct NodeJob {
    pub node_id: i64,
    pub run: Box<dyn Fn() -> BoxFuture<'static, String> + Send + Sync>,
}

/// 按 nodeID 分组后并行执行，返回按节点首次出现顺序、组内顺序收集的非空失败文案。
pub async fn run_node_jobs(jobs: Vec<NodeJob>) -> Vec<String> {
    if jobs.is_empty() {
        return Vec::new();
    }

    let mut order: Vec<i64> = Vec::new();
    let mut groups: std::collections::HashMap<i64, Vec<Box<dyn Fn() -> BoxFuture<'static, String> + Send + Sync>>> =
        std::collections::HashMap::new();
    for job in jobs {
        if !groups.contains_key(&job.node_id) {
            order.push(job.node_id);
        }
        groups.entry(job.node_id).or_default().push(job.run);
    }

    let groups_ref = &groups;
    let mut results: Vec<(usize, Vec<String>)> =
        futures_util::stream::iter(order.into_iter().enumerate())
            .map(move |(i, node_id)| {
                let groups = groups_ref;
                async move {
                    let mut msgs = Vec::new();
                    if let Some(fns) = groups.get(&node_id) {
                        for run in fns {
                            let m = run().await;
                            if !m.is_empty() {
                                msgs.push(m);
                            }
                        }
                    }
                    (i, msgs)
                }
            })
            .buffer_unordered(BATCH_CONCURRENCY)
            .collect()
            .await;
    results.sort_by_key(|(i, _)| *i);

    let mut out = Vec::new();
    for (_, mut msgs) in results {
        out.append(&mut msgs);
    }
    out
}
