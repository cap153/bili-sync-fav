use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use clap::Parser;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};
use log::{error, warn};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration};

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

// --- 命令行参数 ---
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value = "config.toml")]
    config: PathBuf,
}

// --- 邮件通知函数 ---
fn send_notification_email(smtp_config: &Smtp, subject: &str, body: &str) -> Result<()> {
    // 检查 SMTP 密码是否配置，如果没有则不发送
    let Some(password) = smtp_config.sender_password.as_deref() else {
        warn!("SMTP 的 SENDER_PASSWORD 未配置，跳过邮件通知。");
        return Ok(());
    };

    if password.is_empty() || password == "null" {
        warn!("SMTP 的 SENDER_PASSWORD 为空或 'null'，跳过邮件通知。");
        return Ok(());
    }

    println!("正在尝试发送邮件通知...");

    let email = Message::builder()
        .from(format!("BiliBili下载助手 <{}>", smtp_config.sender_email).parse()?)
        .to(smtp_config.recipient_email.parse()?)
        .subject(subject)
        .body(String::from(body))?;

    // 从 SMTP URL 中解析出 host
    let host_part = smtp_config
        .url
        .strip_prefix("smtps://")
        .or_else(|| smtp_config.url.strip_prefix("smtp://"))
        .unwrap_or(&smtp_config.url);

    let smtp_host = host_part.split(':').next().unwrap_or(host_part);

    if smtp_host.is_empty() {
        return Err(anyhow::anyhow!(
            "无法从 SMTP_URL '{}' 中解析出主机名",
            smtp_config.url
        ));
    }

    let creds = Credentials::new(smtp_config.sender_email.clone(), password.to_string());

    // 创建 SMTP transport
    let mailer = SmtpTransport::relay(smtp_host)?.credentials(creds).build();

    // 发送邮件
    match mailer.send(&email) {
        Ok(_) => {
            println!("邮件通知发送成功！");
            Ok(())
        }
        Err(e) => {
            Err(e).with_context(|| "邮件通知发送失败！请检查 config.toml 中的 SMTP 配置和网络。")
        }
    }
}

// --- 新的 Cookie 检查函数，使用库调用 ---
async fn check_cookie_and_notify(config: &Config) -> Result<()> {
    println!("正在检查Cookie状态...");
    // 直接调用库函数，不再解析输出了！
    // 如果函数返回 Ok，说明检查通过。如果返回 Err，说明失败。
    match fav_bili::check_all().await {
        Ok(_) => {
            println!("Cookie验证通过，即将开始下一轮检查。");
            println!("==================================================");
            Ok(())
        }
        Err(e) => {
            // 获取当前的本地时间
            let now: DateTime<Local> = Local::now();
            // 格式化时间为 "年-月-日 时:分:秒"
            let formatted_time = now.format("%Y-%m-%d %H:%M:%S").to_string();

            let error_msg = format!(
                "B站Cookie已过期或验证失败，脚本已停止运行。请立即更新 {} 并重新运行脚本。\n错误详情: {}\n错误时间: {}",
                "config.toml", e, formatted_time // <-- 使用格式化后的时间
            );
            error!("{}", error_msg);

            if let Err(e_mail) = send_notification_email(
                &config.smtp,
                "【紧急】BiliBili Cookie 过期提醒",
                &error_msg,
            ) {
                error!("{:?}", e_mail);
            }

            Err(anyhow::anyhow!(error_msg))
        }
    }
}

// 使用 tokio::main 宏，因为 fav_bili 都是 async 函数
#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let args = Args::parse();
    let original_path = std::env::current_dir()?; // 保存原始工作目录

    println!("正在从 {:?} 读取配置...", &args.config);
    let config_content = std::fs::read_to_string(&args.config)
        .with_context(|| format!("无法读取配置文件: {:?}", &args.config))?;
    let config: Config =
        toml::from_str(&config_content).with_context(|| "解析 config.toml 文件失败")?;

    // --- 登录B站 (库调用) ---
    println!("正在使用Cookie登录...");
    let cookie_str = format!(
        "SESSDATA={};bili_jct={};buvid3={};DedeUserID={};ac_time_value={}", // 注意格式变为分号或空格分隔
        config.credential.sessdata,
        config.credential.bili_jct,
        config.credential.buvid3,
        config.credential.dedeuserid,
        config.credential.ac_time_value,
    );
    // 直接调用 usecookies 函数
    if let Err(e) = fav_bili::usecookies(cookie_str).await {
        // 登录失败，构造错误信息
        // 获取当前的本地时间
        let now: DateTime<Local> = Local::now();
        // 格式化时间为 "年-月-日 时:分:秒"
        let formatted_time = now.format("%Y-%m-%d %H:%M:%S").to_string();
        let error_msg = format!(
        "初次使用Cookie登录失败，脚本已停止运行。\n请检查 config.toml 中的 Cookie 是否正确或已过期。\n错误详情: {}\n错误时间: {}",
        e, formatted_time
    );
        error!("{}", error_msg); // 使用 log 记录

        // 发送邮件通知
        if let Err(e_mail) = send_notification_email(
            &config.smtp,
            "【紧急】BiliBili 登录失败提醒", // 使用不同的主题
            &error_msg,
        ) {
            error!("发送邮件通知失败: {:?}", e_mail);
        }

        // 返回最终的错误，终止程序
        return Err(anyhow::anyhow!(error_msg));
    }

    // 初始 Cookie 检查
    check_cookie_and_notify(&config).await?;

    println!("登录成功，正在全局拉取元数据...");
    // 直接调用 fetch 函数
    fav_bili::fetch(false).await.context("全局 fetch 失败")?; // prune=false
    println!("==================================================");

    // ---  创建目录 ---
    println!("正在初始化本地目录和状态文件...");
    for (_id, dir_str) in &config.favorite_list {
        // println!("处理收藏夹 ID: {} -> 目录: {}", id, dir_str);
        let dir_path = Path::new(dir_str);
        std::fs::create_dir_all(dir_path)?;
    }
    println!("初始化完成！");
    println!("==================================================");

    // --- 循环检查和下载 ---
    loop {
        println!("开始新一轮的视频下载检查...");
        for (id_str, dir_str) in &config.favorite_list {
            let fav_id = id_str
                .parse::<i64>()
                .with_context(|| format!("无效的收藏夹ID: {}", id_str))?;
            println!("--- 正在处理收藏夹: {} ({}) ---", fav_id, dir_str);

            let dir_path = Path::new(dir_str);

            // 切换工作目录，模拟 `(cd "$dir"; ...)`
            std::env::set_current_dir(dir_path)
                .with_context(|| format!("无法切换到目录: {:?}", dir_path))?;

            // 使用库调用，在当前（已切换的）目录中执行操作
            println!("  -> 激活收藏夹 {}", fav_id);
            fav_bili::activate_set(fav_id).await?;

            println!("  -> 检查更新...");
            fav_bili::fetch(false).await?; // prune=false

            println!("  -> 拉取视频...");
            fav_bili::pull().await?;

            println!("  -> 取消激活收藏夹 {}", fav_id);
            fav_bili::deactivate_set(fav_id).await?;

            // 切换回原始工作目录，保持状态一致性
            std::env::set_current_dir(&original_path)?;

            println!("--- 收藏夹 {} 处理完毕 ---", fav_id);
        }

        println!("==================================================");
        println!("所有收藏夹处理完毕，脚本将休眠 {} 秒...", config.interval);
        tokio::time::sleep(Duration::from_secs(config.interval)).await;

        if let Err(e) = check_cookie_and_notify(&config).await {
            error!("致命错误，程序退出: {:?}", e);
            return Err(e);
        }
    }
}
