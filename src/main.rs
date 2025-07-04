use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use clap::Parser;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};
use log::{error, info, warn};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

// --- 数据结构 (保持不变) ---
#[derive(Deserialize, Debug)]
struct Config {
    interval: u64,
    credential: Credential,
    #[serde(rename = "SMTP")]
    smtp: Smtp,
    favorite_list: HashMap<String, String>,
}

#[derive(Deserialize, Debug)]
struct Credential {
    sessdata: String,
    bili_jct: String,
    buvid3: String,
    dedeuserid: String,
    ac_time_value: String,
}

#[derive(Deserialize, Debug)]
struct Smtp {
    #[serde(rename = "SMTP_URL")]
    url: String,
    #[serde(rename = "SENDER_EMAIL")]
    sender_email: String,
    #[serde(rename = "SENDER_PASSWORD")]
    sender_password: Option<String>,
    #[serde(rename = "RECIPIENT_EMAIL")]
    recipient_email: String,
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value = "config.toml")]
    config: PathBuf,
}

struct DirectoryGuard {
    original_path: PathBuf,
}

impl DirectoryGuard {
    /// 创建一个新的 Guard，它会保存当前目录，然后切换到新目录。
    fn new<P: AsRef<Path>>(new_path: P) -> Result<Self> {
        let original_path = std::env::current_dir()?;
        std::env::set_current_dir(new_path.as_ref())
            .with_context(|| format!("无法切换到目录: {:?}", new_path.as_ref().as_os_str()))?;
        Ok(Self { original_path })
    }
}

impl Drop for DirectoryGuard {
    /// 当 DirectoryGuard 被销毁时，这个方法会被自动调用。
    fn drop(&mut self) {
        // 尝试切换回原始目录，如果失败则记录一个警告。
        if let Err(e) = std::env::set_current_dir(&self.original_path) {
            warn!("无法切换回原始目录 {:?}: {}", self.original_path, e);
        }
    }
}

// --- 程序入口 ---
#[tokio::main]
async fn main() -> Result<()> {
    // 在初始化 logger 之前加载 .env 文件
    // .ok() 表示即使 .env 文件不存在也不会报错
    dotenv::dotenv().ok(); 

    // 初始化日志记录器
    env_logger::init();

    // 将主要逻辑委托给 run 函数
    // 这样 main 函数只负责启动和最终的错误处理
    if let Err(e) = run().await {
        error!("程序因严重错误而终止: {:?}", e);
        // 确保非零退出码
        std::process::exit(1);
    }

    Ok(())
}

/// 应用程序的主逻辑函数
async fn run() -> Result<()> {
    let args = Args::parse();

    info!("正在从 {:?} 读取配置...", &args.config);
    let config_content = std::fs::read_to_string(&args.config)
        .with_context(|| format!("无法读取配置文件: {:?}", &args.config))?;
    let config: Config =
        toml::from_str(&config_content).with_context(|| "解析 config.toml 文件失败")?;

    // 步骤1: 登录B站
    login_bilibili(&config).await?;

    // 步骤2: 准备工作目录
    prepare_directories(&config).await?;

    // 步骤3: 进入主同步循环
    run_sync_loop(&config).await?;

    Ok(())
}

// --- 核心功能函数 ---

/// 使用配置中的Cookie登录B站
async fn login_bilibili(config: &Config) -> Result<()> {
    info!("正在使用Cookie登录B站...");
    let cookie_str = format!(
        "SESSDATA={};bili_jct={};buvid3={};DedeUserID={};ac_time_value={}",
        config.credential.sessdata,
        config.credential.bili_jct,
        config.credential.buvid3,
        config.credential.dedeuserid,
        config.credential.ac_time_value,
    );

    if let Err(e) = fav_bili::usecookies(cookie_str).await {
        return Err(handle_critical_error(
            config,
            e,
            "【紧急】BiliBili 登录失败提醒",
            "初次使用Cookie登录失败，请检查 config.toml 中的 Cookie 是否正确或已过期。",
        ));
    }
    info!("登录成功！");
    Ok(())
}

/// 根据配置创建所有需要的收藏夹目录
async fn prepare_directories(config: &Config) -> Result<()> {
    info!("正在初始化本地目录和状态文件...");
    for dir_str in config.favorite_list.values() {
        let dir_path = Path::new(dir_str);
        std::fs::create_dir_all(dir_path).with_context(|| format!("创建目录失败: {}", dir_str))?;
    }
    info!("目录初始化完成！");
    Ok(())
}

/// 主同步循环，定期检查和下载视频
async fn run_sync_loop(config: &Config) -> Result<()> {
    // 首次运行时，先进行一次全局元数据拉取
    info!("正在进行初次全局元数据拉取...");
    fav_bili::fetch(false)
        .await
        .context("全局元数据拉取(fetch)失败")?;
    info!("==================================================");

    loop {
        // 在每一轮循环开始前，检查Cookie状态
        check_cookie_and_notify(config).await?;

        info!("开始新一轮的视频下载检查...");
        for (id_str, dir_str) in &config.favorite_list {
            let fav_id = id_str
                .parse::<i64>()
                .with_context(|| format!("无效的收藏夹ID: {}", id_str))?;

            let dir_path = Path::new(dir_str);

            // 调用专门处理单个收藏夹的函数
            if let Err(e) = process_favorite_list(fav_id, dir_path).await {
                error!("处理收藏夹 {} ({}) 时发生错误: {:?}", fav_id, dir_str, e);
            }
        }

        info!("==================================================");
        info!("所有收藏夹处理完毕，bili-sync-fav将休眠 {} 秒...", config.interval);
        tokio::time::sleep(Duration::from_secs(config.interval)).await;
    }
}

/// 处理单个收藏夹的同步逻辑
async fn process_favorite_list(fav_id: i64, path: &Path) -> Result<()> {
    info!("--- 正在处理收藏夹: {} ({:?}) ---", fav_id, path);

    // 创建 Guard。如果切换目录失败，`?` 会立即返回错误。
    // `_guard` 这个变量名表示我们只关心它的生命周期，不直接使用它。
    let _guard = DirectoryGuard::new(path)?;

    info!("  -> 激活收藏夹 {}", fav_id);
    fav_bili::activate_set(fav_id).await?;

    info!("  -> 检查更新...");
    fav_bili::fetch(false).await?;

    info!("  -> 拉取视频...");
    fav_bili::pull().await?;

    info!("  -> 取消激活收藏夹 {}", fav_id);
    fav_bili::deactivate_set(fav_id).await?;
    
    info!("--- 收藏夹 {} 处理完毕 ---", fav_id);
    Ok(())
}

// --- 辅助与通知函数 ---

/// 检查Cookie有效性，如果失败则发送通知并返回错误
async fn check_cookie_and_notify(config: &Config) -> Result<()> {
    info!("正在检查Cookie状态...");
    if let Err(e) = fav_bili::check_all().await {
        return Err(handle_critical_error(
            config,
            e,
            "【紧急】BiliBili Cookie 过期提醒",
            "B站Cookie已过期或验证失败，bili-sync-fav已停止运行。请立即更新Cookie。",
        ));
    }
    info!("Cookie验证通过。");
    Ok(())
}

/// 统一处理严重错误：记录日志、发送邮件，并返回一个格式化的Error
fn handle_critical_error(
    config: &Config,
    error: anyhow::Error,
    subject: &str,
    context_msg: &str,
) -> anyhow::Error {
    let now: DateTime<Local> = Local::now();
    let formatted_time = now.format("%Y-%m-%d %H:%M:%S").to_string();

    let error_msg = format!(
        "{}\n错误详情: {}\n错误时间: {}",
        context_msg, error, formatted_time
    );

    error!("{}", error_msg);

    if let Err(e_mail) = send_notification_email(&config.smtp, subject, &error_msg) {
        error!("发送邮件通知失败: {:?}", e_mail);
    }

    anyhow::anyhow!(error_msg)
}

/// 发送邮件通知 (基本保持不变，稍作调整)
fn send_notification_email(smtp_config: &Smtp, subject: &str, body: &str) -> Result<()> {
    let Some(password) = smtp_config.sender_password.as_deref() else {
        warn!("SMTP 的 SENDER_PASSWORD 未配置，跳过邮件通知。");
        return Ok(());
    };

    if password.is_empty() || password.eq_ignore_ascii_case("null") {
        warn!("SMTP 的 SENDER_PASSWORD 为空或 'null'，跳过邮件通知。");
        return Ok(());
    }

    info!("正在尝试发送邮件通知...");

    let email = Message::builder()
        .from(format!("BiliBili下载助手 <{}>", smtp_config.sender_email).parse()?)
        .to(smtp_config.recipient_email.parse()?)
        .subject(subject)
        .body(String::from(body))?;

    let smtp_host = smtp_config
        .url
        .strip_prefix("smtps://")
        .or_else(|| smtp_config.url.strip_prefix("smtp://"))
        .and_then(|s| s.split(':').next())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow::anyhow!("无法从 SMTP_URL '{}' 中解析出主机名", smtp_config.url))?;

    let creds = Credentials::new(smtp_config.sender_email.clone(), password.to_string());
    let mailer = SmtpTransport::relay(smtp_host)?.credentials(creds).build();

    mailer
        .send(&email)
        .with_context(|| "邮件通知发送失败！请检查 SMTP 配置和网络。")?;

    info!("邮件通知发送成功！");
    Ok(())
}
