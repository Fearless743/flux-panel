use serde::{Deserialize, Serialize};
use sqlx::{FromRow, QueryBuilder, Sqlite, SqlitePool};
use std::collections::{HashMap, HashSet};

use crate::error::{AppError, AppResult};
use crate::model::{StatisticsFlow, User, UserTunnel};
use crate::response::now_ms;

pub async fn find_by_username(pool: &SqlitePool, username: &str) -> AppResult<Option<User>> {
    Ok(
        sqlx::query_as::<_, User>("SELECT * FROM user WHERE user = ? LIMIT 1")
            .bind(username)
            .fetch_optional(pool)
            .await?,
    )
}

pub async fn find_by_id(pool: &SqlitePool, id: i64) -> AppResult<Option<User>> {
    Ok(sqlx::query_as::<_, User>("SELECT * FROM user WHERE id = ? LIMIT 1")
        .bind(id)
        .fetch_optional(pool)
        .await?)
}

pub async fn count_by_username(
    pool: &SqlitePool,
    username: &str,
    exclude_id: i64,
) -> AppResult<i64> {
    let n = if exclude_id > 0 {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(1) FROM user WHERE user = ? AND id <> ?",
        )
        .bind(username)
        .bind(exclude_id)
        .fetch_one(pool)
        .await?
    } else {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(1) FROM user WHERE user = ?")
            .bind(username)
            .fetch_one(pool)
            .await?
    };
    Ok(n)
}

pub async fn list_non_admin(pool: &SqlitePool) -> AppResult<Vec<User>> {
    Ok(sqlx::query_as::<_, User>("SELECT * FROM user WHERE role_id <> 0 ORDER BY id")
        .fetch_all(pool)
        .await?)
}

pub async fn create(pool: &SqlitePool, u: &mut User) -> AppResult<()> {
    let res = sqlx::query(
        "INSERT INTO user (user, pwd, role_id, exp_time, flow, in_flow, out_flow, flow_reset_time, num, created_time, updated_time, status)
         VALUES (?, ?, ?, ?, ?, 0, 0, ?, ?, ?, ?, ?)",
    )
    .bind(&u.user)
    .bind(&u.pwd)
    .bind(u.role_id)
    .bind(u.exp_time)
    .bind(u.flow)
    .bind(u.flow_reset_time)
    .bind(u.num)
    .bind(u.created_time)
    .bind(u.updated_time)
    .bind(u.status)
    .execute(pool)
    .await?;
    u.id = res.last_insert_rowid();
    Ok(())
}

pub async fn update(pool: &SqlitePool, u: &User, update_pwd: bool) -> AppResult<()> {
    if update_pwd {
        sqlx::query(
            "UPDATE user SET user=?, pwd=?, flow=?, num=?, exp_time=?, flow_reset_time=?, status=?, updated_time=? WHERE id=?",
        )
        .bind(&u.user)
        .bind(&u.pwd)
        .bind(u.flow)
        .bind(u.num)
        .bind(u.exp_time)
        .bind(u.flow_reset_time)
        .bind(u.status)
        .bind(u.updated_time)
        .bind(u.id)
        .execute(pool)
        .await?;
    } else {
        sqlx::query(
            "UPDATE user SET user=?, flow=?, num=?, exp_time=?, flow_reset_time=?, status=?, updated_time=? WHERE id=?",
        )
        .bind(&u.user)
        .bind(u.flow)
        .bind(u.num)
        .bind(u.exp_time)
        .bind(u.flow_reset_time)
        .bind(u.status)
        .bind(u.updated_time)
        .bind(u.id)
        .execute(pool)
        .await?;
    }
    Ok(())
}

pub async fn update_password_and_username(
    pool: &SqlitePool,
    id: i64,
    username: &str,
    pwd_md5: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE user SET user=?, pwd=?, updated_time=? WHERE id=?")
        .bind(username)
        .bind(pwd_md5)
        .bind(now_ms())
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn reset_user_flow(pool: &SqlitePool, id: i64) -> AppResult<()> {
    sqlx::query("UPDATE user SET in_flow=0, out_flow=0, updated_time=? WHERE id=?")
        .bind(now_ms())
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete(pool: &SqlitePool, id: i64) -> AppResult<()> {
    sqlx::query("DELETE FROM user WHERE id=?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_user_tunnels(pool: &SqlitePool, user_id: i64) -> AppResult<()> {
    sqlx::query("DELETE FROM user_tunnel WHERE user_id=?")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_statistics_flow(pool: &SqlitePool, user_id: i64) -> AppResult<()> {
    sqlx::query("DELETE FROM statistics_flow WHERE user_id=?")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_forwards_by_user(pool: &SqlitePool, user_id: i64) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    let ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM forward WHERE user_id=?")
        .bind(user_id)
        .fetch_all(&mut *tx)
        .await?;
    if !ids.is_empty() {
        let mut qb: QueryBuilder<Sqlite> =
            QueryBuilder::new("DELETE FROM forward_port WHERE forward_id IN (");
        let mut sep = qb.separated(", ");
        for id in &ids {
            sep.push_bind(id);
        }
        sep.push_unseparated(")");
        qb.build().execute(&mut *tx).await?;
    }
    sqlx::query("DELETE FROM forward WHERE user_id=?")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

// ---- package info ----

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct PackageUserTunnelDetail {
    pub id: i64,
    pub user_id: i64,
    pub tunnel_id: i64,
    pub tunnel_name: Option<String>,
    pub tunnel_flow: Option<i64>,
    pub flow: i64,
    pub in_flow: i64,
    pub out_flow: i64,
    pub num: i64,
    pub flow_reset_time: i64,
    pub exp_time: i64,
    pub speed_id: Option<i64>,
    pub speed_limit_name: Option<String>,
    pub speed: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct UserForwardDetail {
    pub id: i64,
    pub name: String,
    pub tunnel_id: i64,
    pub tunnel_name: Option<String>,
    pub in_ip: Option<String>,
    #[sqlx(default)]
    pub in_port: Option<i32>,
    pub remote_addr: String,
    pub in_flow: i64,
    pub out_flow: i64,
    pub status: i32,
    pub created_time: i64,
}

pub async fn get_user_tunnel_details(
    pool: &SqlitePool,
    user_id: i64,
) -> AppResult<Vec<PackageUserTunnelDetail>> {
    Ok(sqlx::query_as::<_, PackageUserTunnelDetail>(
        "SELECT
            ut.id,
            ut.user_id,
            ut.tunnel_id,
            t.name as tunnel_name,
            t.flow as tunnel_flow,
            ut.flow,
            ut.in_flow,
            ut.out_flow,
            ut.num,
            ut.flow_reset_time,
            ut.exp_time,
            ut.speed_id,
            sl.name as speed_limit_name,
            sl.speed
        FROM user_tunnel ut
        LEFT JOIN tunnel t ON ut.tunnel_id = t.id
        LEFT JOIN speed_limit sl ON ut.speed_id = sl.id
        WHERE ut.user_id = ?
        ORDER BY ut.id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?)
}

pub async fn get_user_forward_details(
    pool: &SqlitePool,
    user_id: i64,
) -> AppResult<Vec<UserForwardDetail>> {
    Ok(sqlx::query_as::<_, UserForwardDetail>(
        "SELECT
            f.id,
            f.name,
            f.tunnel_id,
            t.name as tunnel_name,
            t.in_ip as in_ip,
            f.remote_addr,
            f.in_flow,
            f.out_flow,
            f.status,
            f.created_time
        FROM forward f
        LEFT JOIN tunnel t ON f.tunnel_id = t.id
        WHERE f.user_id = ?
        ORDER BY f.created_time DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?)
}

#[derive(Debug, Clone, FromRow)]
struct ForwardPortRow {
    forward_id: i64,
    port: i32,
    server_ip: String,
}

pub async fn fill_forward_in_ip_and_port(
    pool: &SqlitePool,
    forwards: &mut [UserForwardDetail],
) -> AppResult<()> {
    if forwards.is_empty() {
        return Ok(());
    }
    let ids: Vec<i64> = forwards.iter().map(|f| f.id).collect();
    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(
        "SELECT fp.forward_id, fp.port, IFNULL(n.server_ip,'') as server_ip
         FROM forward_port fp
         LEFT JOIN node n ON fp.node_id = n.id
         WHERE fp.forward_id IN (",
    );
    let mut sep = qb.separated(", ");
    for id in &ids {
        sep.push_bind(id);
    }
    sep.push_unseparated(") ORDER BY fp.id");
    let ports: Vec<ForwardPortRow> = qb.build_query_as().fetch_all(pool).await?;

    let mut by_fwd: HashMap<i64, Vec<ForwardPortRow>> = HashMap::new();
    for p in ports {
        by_fwd.entry(p.forward_id).or_default().push(p);
    }

    for f in forwards.iter_mut() {
        let Some(fps) = by_fwd.get(&f.id) else {
            continue;
        };
        if fps.is_empty() {
            continue;
        }
        let use_tunnel_in_ip = f
            .in_ip
            .as_deref()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false);
        let mut ip_port_set: Vec<String> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        let mut add = |s: String| {
            if seen.insert(s.clone()) {
                ip_port_set.push(s);
            }
        };

        if use_tunnel_in_ip {
            let in_ip = f.in_ip.clone().unwrap_or_default();
            let mut uniq_ip: Vec<String> = Vec::new();
            let mut ip_seen: HashSet<String> = HashSet::new();
            for part in in_ip.split(',') {
                let p = part.trim();
                if !p.is_empty() && ip_seen.insert(p.to_string()) {
                    uniq_ip.push(p.to_string());
                }
            }
            let mut uniq_ports: Vec<i32> = Vec::new();
            let mut port_seen: HashSet<i32> = HashSet::new();
            for p in fps {
                if port_seen.insert(p.port) {
                    uniq_ports.push(p.port);
                }
            }
            for ip in &uniq_ip {
                for port in &uniq_ports {
                    add(format!("{}:{}", ip, port));
                }
            }
            if let Some(p) = uniq_ports.first() {
                f.in_port = Some(*p);
            }
        } else {
            for p in fps {
                if !p.server_ip.is_empty() {
                    add(format!("{}:{}", p.server_ip, p.port));
                }
            }
            f.in_port = Some(fps[0].port);
        }
        if !ip_port_set.is_empty() {
            f.in_ip = Some(ip_port_set.join(","));
        }
    }
    Ok(())
}

pub async fn get_last_24_hours_flow(
    pool: &SqlitePool,
    user_id: i64,
) -> AppResult<Vec<StatisticsFlow>> {
    let mut list = sqlx::query_as::<_, StatisticsFlow>(
        "SELECT * FROM statistics_flow WHERE user_id = ? ORDER BY id DESC LIMIT 24",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    if list.len() < 24 {
        let mut start_hour = if list.is_empty() {
            chrono::Local::now().hour() as i32
        } else {
            parse_hour(&list[list.len() - 1].time) - 1
        };
        while list.len() < 24 {
            if start_hour < 0 {
                start_hour = 23;
            }
            list.push(StatisticsFlow {
                user_id,
                time: format!("{:02}:00", start_hour),
                ..Default::default()
            });
            start_hour -= 1;
        }
    }
    Ok(list)
}

use chrono::Timelike;

fn parse_hour(time_str: &str) -> i32 {
    if let Some((h, _)) = time_str.split_once(':') {
        if let Ok(v) = h.parse::<i32>() {
            return v;
        }
    }
    chrono::Local::now().hour() as i32
}

pub async fn find_user_tunnel_by_id(
    pool: &SqlitePool,
    id: i64,
) -> AppResult<Option<UserTunnel>> {
    Ok(
        sqlx::query_as::<_, UserTunnel>("SELECT * FROM user_tunnel WHERE id = ? LIMIT 1")
            .bind(id)
            .fetch_optional(pool)
            .await?,
    )
}

pub async fn reset_user_tunnel_flow(pool: &SqlitePool, id: i64) -> AppResult<()> {
    sqlx::query("UPDATE user_tunnel SET in_flow=0, out_flow=0 WHERE id=?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub fn _unused(_: AppError) {}
