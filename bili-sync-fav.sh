#!/bin/bash

# 这个脚本的功能和项目的功能一样，分别调用yq解析toml和fav下载视频，性能可能差一点
# 脚本出错时立即退出
set -e

# --- 初始设置 ---
# 确保 yq 和 fav_bili 可执行
# https://github.com/mikefarah/yq
curl -L "https://github.com/mikefarah/yq/releases/download/v4.45.4/yq_linux_amd64" -o yq_linux_amd64
chmod +x ./yq_linux_amd64
# fav_bili需要自行编译并放到脚本同一目录
chmod +x ./fav_bili

CONFIG_FILE="config.toml"
FAV_BILI_EXEC="./fav_bili"
YQ_EXEC="./yq_linux_amd64"

# 检查配置文件是否存在
if [ ! -f "$CONFIG_FILE" ]; then
    echo "错误: 配置文件 $CONFIG_FILE 不存在。"
    exit 1
fi

echo "正在从 $CONFIG_FILE 读取配置..."

# 时间间隔，单位是秒
interval=$($YQ_EXEC '.interval' "$CONFIG_FILE")
# 读取用于登陆的cookie
sessdata=$($YQ_EXEC '.credential.sessdata' "$CONFIG_FILE")
bili_jct=$($YQ_EXEC '.credential.bili_jct' "$CONFIG_FILE")
buvid3=$($YQ_EXEC '.credential.buvid3' "$CONFIG_FILE")
dedeuserid=$($YQ_EXEC '.credential.dedeuserid' "$CONFIG_FILE")

# --- 读取 SMTP 配置 ---
SMTP_URL=$($YQ_EXEC '.SMTP.SMTP_URL' "$CONFIG_FILE")
SENDER_EMAIL=$($YQ_EXEC '.SMTP.SENDER_EMAIL' "$CONFIG_FILE")
SENDER_PASSWORD=$($YQ_EXEC '.SMTP.SENDER_PASSWORD' "$CONFIG_FILE")
RECIPIENT_EMAIL=$($YQ_EXEC '.SMTP.RECIPIENT_EMAIL' "$CONFIG_FILE")

# --- 邮件通知函数 ---
# 参数1: 邮件主题
# 参数2: 邮件正文
send_notification_email() {
    local subject="$1"
    local body="$2"

    # 检查 SMTP 是否配置，特别是密码。如果未配置，则只打印警告，不发送邮件。
    if [ -z "$SENDER_PASSWORD" ] || [ "$SENDER_PASSWORD" = "null" ]; then
        echo "警告: SMTP 的 SENDER_PASSWORD 未配置，跳过邮件通知。"
        return
    fi

    echo "正在尝试发送邮件通知..."

    # 使用 printf 动态创建邮件内容，并通过管道传给 curl
    # From 和 To 最好使用 "昵称 <邮箱地址>" 的格式
    # Subject 需要注意 UTF-8 编码
    # 邮件头和正文之间必须有空行
    local mail_content
    mail_content=$(printf "From: \"BiliBili下载助手\" <%s>\nTo: <%s>\nSubject: %s\nContent-Type: text/plain; charset=utf-8\n\n%s" \
        "$SENDER_EMAIL" "$RECIPIENT_EMAIL" "$subject" "$body")

    # 使用 curl 发送邮件
    # -sS: silent 模式，但在出错时仍然显示错误信息
    # --upload-file - : 从标准输入读取内容
    # 注意：这里的 echo 需要用双引号 "$mail_content" 以保留换行符
    echo "$mail_content" | curl -sS --url "$SMTP_URL" \
         --ssl-reqd \
         --user "$SENDER_EMAIL:$SENDER_PASSWORD" \
         --mail-from "$SENDER_EMAIL" \
         --mail-rcpt "$RECIPIENT_EMAIL" \
         --upload-file -

    if [ $? -eq 0 ]; then
        echo "邮件通知发送成功！"
    else
        echo "错误: 邮件通知发送失败！请检查 config.toml 中的 SMTP 配置和网络。"
        # 即使邮件发送失败，脚本也应该继续按原计划退出，所以这里不加 exit 1
    fi
}

# --- 1. 登录B站 ---
echo "正在使用Cookie登录..."
$FAV_BILI_EXEC auth usecookies SESSDATA=$sessdata bili_jct=$bili_jct buvid3=$buvid3 DedeUserID=$dedeuserid
# 检查登录状态，如果失败则发送邮件并退出
if ! $FAV_BILI_EXEC auth check -a | grep -q "Check passed"; then
	error_msg="B站Cookie已过期，脚本已停止运行。请立即更新 $CONFIG_FILE 并重新运行脚本。错误时间: $(date)"
	echo "错误: $error_msg"
    # --- 修改：调用邮件通知函数 ---
    send_notification_email "【紧急】BiliBili Cookie 过期提醒" "$error_msg"
	exit 1
else
	echo "Cookie验证通过，即将开始下一轮检查。"
	echo "=================================================="
fi
echo "登录成功！"
# 获取所有收藏夹信息
$FAV_BILI_EXEC fetch
echo "=================================================="

# --- 2. 创建目录 & 3. 创建软链接 ---
echo "正在初始化本地目录和状态文件..."
# 使用 yq 读取所有收藏夹ID
$YQ_EXEC '.favorite_list | to_entries | .[] | [.key, .value] | @tsv' "$CONFIG_FILE" | while IFS=$'\t' read -r id dir; do
    echo "处理收藏夹 ID: $id -> 目录: $dir"
    # 创建目录，-p 选项确保如果目录已存在也不会报错
    mkdir -p "$dir"
done
echo "初始化完成！"
echo "=================================================="

# --- 4 & 5. 循环检查和下载 ---
while true; do
    echo "开始新一轮的视频下载检查..."
    
    # 再次遍历所有收藏夹
$YQ_EXEC '.favorite_list | to_entries | .[] | [.key, .value] | @tsv' "$CONFIG_FILE" | while IFS=$'\t' read -r id dir; do
        
        echo "--- 正在处理收藏夹: $id ($dir) ---"
        
        # 进入对应的目录执行操作
        # 使用子shell () 可以确保执行后自动返回当前目录，避免了 cd ..
        (
            cd "$dir"
            echo "  -> 激活收藏夹 $id"
            ../$FAV_BILI_EXEC activate set "$id"
            echo "  -> 检查更新..."
            ../$FAV_BILI_EXEC fetch
            echo "  -> 拉取视频..."
            ../$FAV_BILI_EXEC pull
            echo "  -> 取消激活收藏夹 $id"
            ../$FAV_BILI_EXEC deactivate set "$id"
        )
        echo "--- 收藏夹 $id 处理完毕 ---"
    done
    echo "=================================================="
    echo "所有收藏夹处理完毕，脚本将休眠 $interval 秒..."
    sleep "$interval"
    echo "休眠结束，正在检查Cookie状态..."
	if ! $FAV_BILI_EXEC auth check -a | grep -q "Check passed"; then
		error_msg="B站Cookie已过期，脚本已停止运行。请立即更新 $CONFIG_FILE 并重新运行脚本。错误时间: $(date)"
		echo "错误: $error_msg"
		# --- 再次调用邮件通知函数 ---
		send_notification_email "【紧急】BiliBili Cookie 过期提醒" "$error_msg"
		exit 1
	else
		echo "Cookie验证通过，即将开始下一轮检查。"
		echo "=================================================="
	fi
done
