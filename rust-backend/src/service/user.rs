use serde::{Deserialize, Serialize};

use crate::auth;
use crate::error::{AppError, AppResult};
use crate::model::{self, StatisticsFlow, User};
use crate::repo::user as repo;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct LoginReq {
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    #[serde(default, rename = "captchaId")]
    pub captcha_id: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResp {
    pub token: String,
    pub name: String,
    pub role_id: i32,
    #[serde(rename = "requirePasswordChange")]
    pub require_password_change: bool,
}

pub async fn login(state: &AppState, req: LoginReq) -> AppResult<LoginResp> {
    if req.username.trim().is_empty() || req.password.trim().is_empty() {
        return Err(AppError::new("账号或密码错误"));
    }
    if crate::service::config::is_captcha_enabled(&state.db).await {
        if req.captcha_id.trim().is_empty()
            || !crate::service::captcha::secondary(req.captcha_id.trim())
        {
            return Err(AppError::new("验证码校验失败"));
        }
    }
    let Some(u) = repo::find_by_username(&state.db, &req.username).await? else {
        return Err(AppError::new("账号或密码错误"));
    };
    if !auth::check_password(&req.password, &u.pwd) {
        return Err(AppError::new("账号或密码错误"));
    }
    if u.status == 0 {
        return Err(AppError::new("账号被停用"));
    }
    // 存量 MD5 登录时先升级为 Argon2id，再签发 token，
    // 避免 updated_time 推进导致刚签发的 token 即失效
    let mut pwd_for_check = u.pwd.clone();
    if auth::needs_rehash(&u.pwd) {
        if let Ok(new_hash) = auth::hash_password(&req.password) {
            if repo::update_password_hash(&state.db, u.id, &new_hash)
                .await
                .is_ok()
            {
                pwd_for_check = new_hash;
            }
        }
    }
    let _ = pwd_for_check;
    let token = state
        .jwt
        .generate(u.id, &u.user, u.role_id)
        .map_err(AppError::new)?;
    // 默认口令仍可登录以便改密，但明确提示；启动日志另有警告
    let require_change = req.username == "admin_user"
        || req.password == "admin_user"
        || u.pwd == auth::md5_hex("admin_user");
    if require_change {
        tracing::warn!("admin 使用默认口令登录，请立即修改");
    }
    Ok(LoginResp {
        token,
        name: u.user,
        role_id: u.role_id,
        require_password_change: require_change,
    })
}

#[derive(Debug, Deserialize)]
pub struct CreateUserReq {
    #[serde(default)]
    pub user: String,
    #[serde(default)]
    pub pwd: String,
    #[serde(default)]
    pub flow: i64,
    #[serde(default)]
    pub num: i64,
    #[serde(default, rename = "expTime")]
    pub exp_time: i64,
    #[serde(default, rename = "flowResetTime")]
    pub flow_reset_time: i64,
    #[serde(default)]
    pub status: Option<i32>,
}

pub async fn create(state: &AppState, req: CreateUserReq) -> AppResult<()> {
    if req.user.trim().is_empty() {
        return Err(AppError::new("用户名不能为空"));
    }
    if let Err(e) = auth::validate_password_strength(&req.pwd) {
        return Err(AppError::new(e));
    }
    if repo::count_by_username(&state.db, &req.user, 0).await? > 0 {
        return Err(AppError::new("用户名已存在"));
    }
    let now = crate::response::now_ms();
    let status = req.status.unwrap_or(1);
    let pwd_hash = auth::hash_password(&req.pwd).map_err(AppError::new)?;
    let mut u = User {
        user: req.user,
        pwd: pwd_hash,
        role_id: 1,
        exp_time: req.exp_time,
        flow: req.flow,
        flow_reset_time: req.flow_reset_time,
        num: req.num,
        created_time: now,
        updated_time: Some(now),
        status,
        ..Default::default()
    };
    repo::create(&state.db, &mut u).await
}

pub async fn list(state: &AppState) -> AppResult<Vec<User>> {
    repo::list_non_admin(&state.db).await
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserReq {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub user: String,
    #[serde(default)]
    pub pwd: String,
    #[serde(default)]
    pub flow: i64,
    #[serde(default)]
    pub num: i64,
    #[serde(default, rename = "expTime")]
    pub exp_time: i64,
    #[serde(default, rename = "flowResetTime")]
    pub flow_reset_time: i64,
    #[serde(default)]
    pub status: Option<i32>,
}

pub async fn update(state: &AppState, req: UpdateUserReq) -> AppResult<()> {
    let Some(mut u) = repo::find_by_id(&state.db, req.id).await? else {
        return Err(AppError::new("用户不存在"));
    };
    if u.role_id == 0 {
        return Err(AppError::new("请不要作死"));
    }
    if repo::count_by_username(&state.db, &req.user, req.id).await? > 0 {
        return Err(AppError::new("用户名已存在"));
    }
    let now = crate::response::now_ms();
    u.user = req.user;
    u.flow = req.flow;
    u.num = req.num;
    u.exp_time = req.exp_time;
    u.flow_reset_time = req.flow_reset_time;
    u.updated_time = Some(now);
    if let Some(status) = req.status {
        u.status = status;
    }
    let update_pwd = !req.pwd.trim().is_empty();
    if update_pwd {
        if let Err(e) = auth::validate_password_strength(&req.pwd) {
            return Err(AppError::new(e));
        }
        u.pwd = auth::hash_password(&req.pwd).map_err(AppError::new)?;
    }
    repo::update(&state.db, &u, update_pwd).await
}

pub async fn delete(state: &AppState, id: i64) -> AppResult<()> {
    let Some(u) = repo::find_by_id(&state.db, id).await? else {
        return Err(AppError::new("用户不存在"));
    };
    if u.role_id == 0 {
        return Err(AppError::new("请不要作死"));
    }
    repo::delete_forwards_by_user(&state.db, id).await?;
    repo::delete_user_tunnels(&state.db, id).await?;
    repo::delete_statistics_flow(&state.db, id).await?;
    repo::delete(&state.db, id).await
}

#[derive(Debug, Serialize)]
pub struct UserInfoDto {
    pub id: i64,
    pub user: String,
    pub status: i32,
    pub flow: i64,
    #[serde(rename = "inFlow")]
    pub in_flow: i64,
    #[serde(rename = "outFlow")]
    pub out_flow: i64,
    pub num: i64,
    #[serde(rename = "expTime")]
    pub exp_time: i64,
    #[serde(rename = "flowResetTime")]
    pub flow_reset_time: i64,
    #[serde(rename = "createdTime")]
    pub created_time: i64,
    #[serde(rename = "updatedTime")]
    pub updated_time: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageDto {
    pub user_info: UserInfoDto,
    pub tunnel_permissions: Vec<repo::PackageUserTunnelDetail>,
    pub forwards: Vec<repo::UserForwardDetail>,
    pub statistics_flows: Vec<StatisticsFlow>,
}

pub async fn package(state: &AppState, user_id: i64) -> AppResult<PackageDto> {
    let Some(u) = repo::find_by_id(&state.db, user_id).await? else {
        return Err(AppError::new("用户不存在"));
    };
    let tunnels = repo::get_user_tunnel_details(&state.db, user_id).await?;
    let mut forwards = repo::get_user_forward_details(&state.db, user_id).await?;
    let stats = repo::get_last_24_hours_flow(&state.db, user_id).await?;
    repo::fill_forward_in_ip_and_port(&state.db, &mut forwards).await?;
    Ok(PackageDto {
        user_info: UserInfoDto {
            id: u.id,
            user: u.user,
            status: u.status,
            flow: u.flow,
            in_flow: u.in_flow,
            out_flow: u.out_flow,
            num: u.num,
            exp_time: u.exp_time,
            flow_reset_time: u.flow_reset_time,
            created_time: u.created_time,
            updated_time: u.updated_time,
        },
        tunnel_permissions: tunnels,
        forwards,
        statistics_flows: stats,
    })
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordReq {
    #[serde(default, rename = "newUsername")]
    pub new_username: String,
    #[serde(default, rename = "currentPassword")]
    pub current_password: String,
    #[serde(default, rename = "newPassword")]
    pub new_password: String,
    #[serde(default, rename = "confirmPassword")]
    pub confirm_password: String,
}

pub async fn update_password(
    state: &AppState,
    user_id: i64,
    req: ChangePasswordReq,
) -> AppResult<()> {
    let Some(u) = repo::find_by_id(&state.db, user_id).await? else {
        return Err(AppError::new("用户不存在"));
    };
    if req.new_password != req.confirm_password {
        return Err(AppError::new("新密码和确认密码不匹配"));
    }
    if !auth::check_password(&req.current_password, &u.pwd) {
        return Err(AppError::new("当前密码错误"));
    }
    if let Err(e) = auth::validate_password_strength(&req.new_password) {
        return Err(AppError::new(e));
    }
    if req.new_username.trim().is_empty() {
        return Err(AppError::new("用户名不能为空"));
    }
    if u.user != req.new_username
        && repo::count_by_username(&state.db, &req.new_username, u.id).await? > 0
    {
        return Err(AppError::new("用户名已存在"));
    }
    repo::update_password_and_username(
        &state.db,
        u.id,
        &req.new_username,
        &auth::hash_password(&req.new_password).map_err(AppError::new)?,
    )
    .await
}

#[derive(Debug, Deserialize)]
pub struct ResetFlowReq {
    #[serde(default)]
    pub id: i64,
    #[serde(default, rename = "type")]
    pub r#type: i32,
}

pub async fn reset(state: &AppState, req: ResetFlowReq) -> AppResult<()> {
    if req.r#type == 1 {
        if repo::find_by_id(&state.db, req.id).await?.is_none() {
            return Err(AppError::new("用户不存在"));
        }
        return repo::reset_user_flow(&state.db, req.id).await;
    }
    if repo::find_user_tunnel_by_id(&state.db, req.id)
        .await?
        .is_none()
    {
        return Err(AppError::new("隧道不存在"));
    }
    repo::reset_user_tunnel_flow(&state.db, req.id).await
}

pub async fn open_api_sub_store(
    state: &AppState,
    user: &str,
    pwd: &str,
    tunnel: &str,
) -> AppResult<String> {
    if user.trim().is_empty() {
        return Err(AppError::new("用户不能为空"));
    }
    if pwd.trim().is_empty() {
        return Err(AppError::new("密码不能为空"));
    }
    let Some(u) = repo::find_by_username(&state.db, user).await? else {
        return Err(AppError::new("鉴权失败"));
    };
    if !auth::check_password(pwd, &u.pwd) {
        return Err(AppError::new("鉴权失败"));
    }
    const GIGA: i64 = 1024 * 1024 * 1024;
    if tunnel.is_empty() || tunnel == "-1" {
        return Ok(build_subscription_header(
            u.out_flow,
            u.in_flow,
            u.flow * GIGA,
            u.exp_time / 1000,
        ));
    }
    if !tunnel.chars().all(|c| c.is_ascii_digit()) {
        return Err(AppError::new("隧道不存在"));
    }
    let tid = tunnel
        .parse::<i64>()
        .map_err(|_| AppError::new("隧道不存在"))?;
    let Some(ut) = repo::find_user_tunnel_by_id(&state.db, tid).await? else {
        return Err(AppError::new("隧道不存在"));
    };
    if ut.user_id != u.id {
        return Err(AppError::new("隧道不存在"));
    }
    Ok(build_subscription_header(
        ut.out_flow,
        ut.in_flow,
        ut.flow * GIGA,
        ut.exp_time / 1000,
    ))
}

fn build_subscription_header(upload: i64, download: i64, total: i64, expire: i64) -> String {
    format!(
        "upload={}; download={}; total={}; expire={}",
        download, upload, total, expire
    )
}

pub fn _unused(_: model::User) {}
