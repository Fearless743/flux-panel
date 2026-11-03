use chrono::{Datelike, Local, TimeZone, Timelike};
use chrono::Duration as ChronoDuration;
use futures_util::stream::{self, StreamExt};
use std::collections::{HashMap, HashSet};
use std::time::Duration;

use crate::db;
use crate::model::{Forward, User, UserTunnel};
use crate::service::flow as flow_svc;
use crate::state::AppState;

const EXPIRE_CONCURRENCY: usize = 8;

/// Start 注册并启动定时任务：每天 00:00:05 重置流量、每小时统计流量、每 5 分钟 WAL checkpoint
pub fn start(state: AppState) {
    // ResetFlow: 每天 00:00:05
    {
        let state = state.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(next_daily_duration(0, 0, 5)).await;
                reset_flow(&state).await;
            }
        });
    }
    // StatisticsFlow: 每小时整点
    {
        let state = state.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(next_hour_duration()).await;
                statistics_flow(&state).await;
            }
        });
    }
    // WAL checkpoint 每 5 分钟
    {
        let state = state.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(next_five_min_duration()).await;
                db::checkpoint(&state.db).await;
            }
        });
    }
    tracing::info!(jobs = "ResetFlow/StatisticsFlow/WAL", "cron started");
}

fn at_least_one_ms(d: Duration) -> Duration {
    d.max(Duration::from_millis(1))
}

fn next_daily_duration(hour: u32, minute: u32, second: u32) -> Duration {
    let now = Local::now();
    let mut date = now.date_naive();
    for _ in 0..3 {
        let Some(naive) = date.and_hms_opt(hour, minute, second) else {
            date += ChronoDuration::days(1);
            continue;
        };
        if let Some(dt) = Local.from_local_datetime(&naive).earliest() {
            if dt > now {
                return at_least_one_ms((dt - now).to_std().unwrap_or(Duration::from_secs(1)));
            }
        }
        date += ChronoDuration::days(1);
    }
    Duration::from_secs(86400)
}

fn next_hour_duration() -> Duration {
    let now = Local::now();
    let base = now
        .with_minute(0)
        .and_then(|t| t.with_second(0))
        .and_then(|t| t.with_nanosecond(0))
        .unwrap_or(now);
    let next = if base > now {
        base
    } else {
        base + ChronoDuration::hours(1)
    };
    at_least_one_ms((next - now).to_std().unwrap_or(Duration::from_secs(1)))
}

fn next_five_min_duration() -> Duration {
    let now = Local::now();
    let base = now
        .with_second(0)
        .and_then(|t| t.with_nanosecond(0))
        .unwrap_or(now);
    let rem = (base.minute() % 5) as i64;
    let mut boundary = base - ChronoDuration::minutes(rem);
    if boundary <= now {
        boundary += ChronoDuration::minutes(5);
    }
    at_least_one_ms((boundary - now).to_std().unwrap_or(Duration::from_secs(1)))
}

async fn reset_flow(state: &AppState) {
    tracing::info!("开始执行流量重置任务");
    let now = Local::now();
    let current_day = now.day() as i32;
    let (next_year, next_month) = if now.month() == 12 {
        (now.year() + 1, 1)
    } else {
        (now.year(), now.month() + 1)
    };
    let last_day = chrono::NaiveDate::from_ymd_opt(next_year, next_month, 1)
        .and_then(|d| d.pred_opt())
        .map(|d| d.day() as i32)
        .unwrap_or(current_day);

    // 两张表重置相互独立，并行执行
    tokio::join!(
        reset_user_flow(state, current_day, last_day),
        reset_user_tunnel_flow(state, current_day, last_day)
    );
    tracing::info!("流量重置任务执行完成");

    // 到期扫描相互独立，并行执行
    tokio::join!(expire_users(state), expire_user_tunnels(state));
    tracing::info!("到期任务执行完成");
}

async fn reset_user_flow(state: &AppState, current_day: i32, last_day: i32) {
    // 单语句批量重置
    let (query, args): (&str, Vec<i32>) = if current_day == last_day {
        (
            "UPDATE user SET in_flow = 0, out_flow = 0
			WHERE flow_reset_time != 0
			  AND (flow_reset_time = ? OR flow_reset_time > ?)",
            vec![current_day, last_day],
        )
    } else {
        (
            "UPDATE user SET in_flow = 0, out_flow = 0
			WHERE flow_reset_time != 0 AND flow_reset_time = ?",
            vec![current_day],
        )
    };
    let mut q = sqlx::query(query);
    for a in &args {
        q = q.bind(*a);
    }
    match q.execute(&state.db).await {
        Ok(res) => {
            let n = res.rows_affected();
            if n == 0 {
                tracing::info!("没有需要重置流量的用户");
            } else {
                tracing::info!(count = n, "用户流量重置成功");
            }
        }
        Err(e) => {
            tracing::warn!(err = %e, "用户流量批量重置失败");
        }
    }
}

async fn reset_user_tunnel_flow(state: &AppState, current_day: i32, last_day: i32) {
    // 单语句批量重置
    let (query, args): (&str, Vec<i32>) = if current_day == last_day {
        (
            "UPDATE user_tunnel SET in_flow = 0, out_flow = 0
			WHERE flow_reset_time != 0
			  AND (flow_reset_time = ? OR flow_reset_time > ?)",
            vec![current_day, last_day],
        )
    } else {
        (
            "UPDATE user_tunnel SET in_flow = 0, out_flow = 0
			WHERE flow_reset_time != 0 AND flow_reset_time = ?",
            vec![current_day],
        )
    };
    let mut q = sqlx::query(query);
    for a in &args {
        q = q.bind(*a);
    }
    match q.execute(&state.db).await {
        Ok(res) => {
            let n = res.rows_affected();
            if n == 0 {
                tracing::info!("没有需要重置流量的用户隧道");
            } else {
                tracing::info!(count = n, "用户隧道流量重置成功");
            }
        }
        Err(e) => {
            tracing::warn!(err = %e, "用户隧道流量批量重置失败");
        }
    }
}

async fn expire_users(state: &AppState) {
    let now = crate::response::now_ms();
    let users = match sqlx::query_as::<_, User>(
        r#"
		SELECT * FROM user
		WHERE role_id != 0 AND status = 1 AND exp_time IS NOT NULL AND exp_time < ?"#,
    )
    .bind(now)
    .fetch_all(&state.db)
    .await
    {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(err = %e, "查询过期用户失败");
            return;
        }
    };
    if users.is_empty() {
        return;
    }

    // 转发一次批量查回按用户分组，暂停并发下发
    let user_ids: Vec<i64> = users.iter().map(|u| u.id).collect();
    let all_forwards: Vec<Forward> = {
        let q = format!(
            "SELECT * FROM forward WHERE user_id IN ({}) AND status = 1",
            placeholders(user_ids.len())
        );
        let mut query = sqlx::query_as::<_, Forward>(&q);
        for id in &user_ids {
            query = query.bind(id);
        }
        query.fetch_all(&state.db).await.unwrap_or_default()
    };
    let mut by_user: HashMap<i64, Vec<Forward>> = HashMap::with_capacity(users.len());
    for f in all_forwards {
        by_user.entry(f.user_id).or_default().push(f);
    }

    let groups: Vec<Vec<Forward>> = users
        .iter()
        .filter_map(|u| by_user.remove(&u.id))
        .filter(|fws| !fws.is_empty())
        .collect();
    stream::iter(groups.into_iter().map(|fws| async move {
        flow_svc::pause_forwards(state, &fws).await;
    }))
    .buffer_unordered(EXPIRE_CONCURRENCY)
    .for_each(|_| async {})
    .await;

    let expired_ids: Vec<i64> = users.iter().map(|u| u.id).collect();
    if !expired_ids.is_empty() {
        let q = format!(
            "UPDATE user SET status = 0 WHERE id IN ({})",
            placeholders(expired_ids.len())
        );
        let mut query = sqlx::query(&q);
        for id in &expired_ids {
            query = query.bind(id);
        }
        let _ = query.execute(&state.db).await;
    }
}

async fn expire_user_tunnels(state: &AppState) {
    let now = crate::response::now_ms();
    let list = match sqlx::query_as::<_, UserTunnel>(
        r#"
		SELECT * FROM user_tunnel
		WHERE status = 1 AND exp_time IS NOT NULL AND exp_time < ?"#,
    )
    .bind(now)
    .fetch_all(&state.db)
    .await
    {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(err = %e, "查询过期隧道失败");
            return;
        }
    };
    if list.is_empty() {
        return;
    }

    // 按 (tunnel,user) 去重后一次 IN 查询全部转发，再分组并发暂停
    let mut seen: HashSet<(i64, i64)> = HashSet::with_capacity(list.len());
    let mut pairs: Vec<(i64, i64)> = Vec::with_capacity(list.len());
    for ut in &list {
        let k = (ut.tunnel_id, ut.user_id);
        if seen.contains(&k) {
            continue;
        }
        seen.insert(k);
        pairs.push(k);
    }

    let all_forwards: Vec<Forward> = if pairs.is_empty() {
        Vec::new()
    } else {
        let conds = vec!["(tunnel_id = ? AND user_id = ?)"; pairs.len()].join(" OR ");
        let sql = format!("SELECT * FROM forward WHERE status = 1 AND ({})", conds);
        let mut query = sqlx::query_as::<_, Forward>(&sql);
        for (tunnel_id, user_id) in &pairs {
            query = query.bind(tunnel_id).bind(user_id);
        }
        query.fetch_all(&state.db).await.unwrap_or_default()
    };
    let mut by_pair: HashMap<(i64, i64), Vec<Forward>> = HashMap::with_capacity(pairs.len());
    for f in all_forwards {
        by_pair
            .entry((f.tunnel_id, f.user_id))
            .or_default()
            .push(f);
    }

    let groups: Vec<Vec<Forward>> = list
        .iter()
        .filter_map(|ut| by_pair.get(&(ut.tunnel_id, ut.user_id)).cloned())
        .filter(|fws| !fws.is_empty())
        .collect();
    stream::iter(groups.into_iter().map(|fws| async move {
        flow_svc::pause_forwards(state, &fws).await;
    }))
    .buffer_unordered(EXPIRE_CONCURRENCY)
    .for_each(|_| async {})
    .await;

    let expired_ids: Vec<i64> = list.iter().map(|ut| ut.id).collect();
    if !expired_ids.is_empty() {
        let q = format!(
            "UPDATE user_tunnel SET status = 0 WHERE id IN ({})",
            placeholders(expired_ids.len())
        );
        let mut query = sqlx::query(&q);
        for id in &expired_ids {
            query = query.bind(id);
        }
        let _ = query.execute(&state.db).await;
    }
}

async fn statistics_flow(state: &AppState) {
    let now = Local::now();
    let ts = now.timestamp_millis();
    // 与 Go `now.Truncate(time.Hour).Format("15:04")` 一致（按绝对时间截断）
    let hour_ms = ts - ts.rem_euclid(3_600_000);
    let hour_string = Local
        .timestamp_millis_opt(hour_ms)
        .single()
        .unwrap_or(now)
        .format("%H:%M")
        .to_string();
    let cutoff = ts - 48 * 60 * 60 * 1000;

    if let Err(e) = sqlx::query("DELETE FROM statistics_flow WHERE created_time < ?")
        .bind(cutoff)
        .execute(&state.db)
        .await
    {
        tracing::warn!(err = %e, "清理统计流量失败");
    }

    let users = match sqlx::query_as::<_, User>("SELECT * FROM user")
        .fetch_all(&state.db)
        .await
    {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(err = %e, "统计流量查询用户失败");
            return;
        }
    };
    if users.is_empty() {
        return;
    }

    // 各用户上一次累计值一次查回
    let mut last_total: HashMap<i64, i64> = HashMap::with_capacity(users.len());
    let last_rows = sqlx::query_as::<_, (i64, i64)>(
        r#"
		SELECT user_id, total_flow FROM statistics_flow
		WHERE id IN (SELECT MAX(id) FROM statistics_flow GROUP BY user_id)"#,
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();
    for (user_id, total_flow) in last_rows {
        last_total.insert(user_id, total_flow);
    }

    struct StatRow {
        user_id: i64,
        flow: i64,
        total: i64,
    }
    let mut rows: Vec<StatRow> = Vec::with_capacity(users.len());
    for u in &users {
        let current_total = u.in_flow + u.out_flow;
        let mut increment = current_total;
        if let Some(last) = last_total.get(&u.id) {
            increment = current_total - last;
            if increment < 0 {
                increment = current_total;
            }
        }
        rows.push(StatRow {
            user_id: u.id,
            flow: increment,
            total: current_total,
        });
    }

    // 多行单语句批量插入（每行 5 变量，分片 150 → 750 < 999 上限）
    for chunk in rows.chunks(150) {
        let sql = format!(
            "INSERT INTO statistics_flow (user_id, flow, total_flow, time, created_time) VALUES {}",
            vec!["(?,?,?,?,?)"; chunk.len()].join(",")
        );
        let mut query = sqlx::query(&sql);
        for r in chunk {
            query = query
                .bind(r.user_id)
                .bind(r.flow)
                .bind(r.total)
                .bind(&hour_string)
                .bind(ts);
        }
        if let Err(e) = query.execute(&state.db).await {
            tracing::warn!(rows = chunk.len(), err = %e, "写入统计流量失败");
        }
    }
}

fn placeholders(n: usize) -> String {
    let mut s = String::with_capacity(n * 2);
    for i in 0..n {
        if i > 0 {
            s.push(',');
        }
        s.push('?');
    }
    s
}
