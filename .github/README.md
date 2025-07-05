- [x] 支持登陆失败或 Cookie 过期发送邮件提醒
- [x] 支持toml格式的配置文件
- [x] 支持收藏夹视频定时检测下载
- [x] 为 Linux 平台提供了立即可用的 Docker 镜像
- [x] 使用数据库保存媒体信息，避免对同个视频的多次请求

# 配置文件

## Cookie信息

当前版本的默认示例文件`config.toml`如下：

```bash
# 这是时间间隔，单位是秒
interval = 43200

# 这是用于登陆的Cookie信息
[credential]
sessdata = ""
bili_jct = ""
buvid3 = ""
dedeuserid = ""
# 打开开发者工具，进入控制台，输入window.localStorage.ac_time_value即可获取值
ac_time_value = ""

# 这是收藏夹的id和对应视频在本地下载的目录
[favorite_list]
<收藏夹id> = "<保存的路径>"
<收藏夹id> = "<保存的路径>"

# 这是邮件的配置(以163为例)
[SMTP]
SMTP_URL="smtps://smtp.163.com:465"
# 开启了SMTP的邮箱
SENDER_EMAIL="test@163.com"
# 重要：这里使用授权码，而不是邮箱密码！
SENDER_PASSWORD="" 
# 收件的邮箱
RECIPIENT_EMAIL="test@qq.com"
```

- `interval`:表示程序每次执行扫描下载的间隔时间，单位为秒。
- `credential`:哔哩哔哩账号的身份凭据，请参考凭据获取[流程获取](https://nemo2011.github.io/bilibili-api/#/get-credential)并对应填写至配置文件中，后续 bili-sync 会在必要时自动刷新身份凭据，不再需要手动管理。推荐使用匿名窗口获取，避免潜在的冲突。
- `favorite_list`:你想要下载的收藏夹与想要保存的位置。简单示例：

```bash
3115878158 = "/home/amtoaer/Downloads/bili-sync/测试收藏夹"
```

## 收藏夹id获取方法

打开想要下载的收藏夹，点击**收藏夹封面旁边**的`播放全部`按钮，

![image](https://github.com/user-attachments/assets/a8a39d76-611d-40cf-9474-8193e26ae3ec)

新打开的标签页可以在地址栏看到“mlxxxxxxx”，只需要后面数字即可（不需要“ml“）

![image](https://github.com/user-attachments/assets/59921d0c-96ee-4fa0-842e-a757c603ccdd)

## SMTP配置

> [!NOTE]
> 配置好SMTP可以在登陆失败或cookie过期时发邮件提醒，这里以开启163邮箱的SMTP为例

登陆163邮箱，点击设置，选择`POP3/SMTP/IMAP`

![image](https://github.com/user-attachments/assets/1d27d05a-04d4-4901-b135-9000e7df2f6e)

点击`POP3/SMTP服务`旁边的开启，顺着步骤一步一步走就行，发送短信后会显示授权码，**记住这个授权码，这个是在你写代码发邮件时代替你邮箱密码的**

![image](https://github.com/user-attachments/assets/bf73a8fc-1476-41ce-8afe-07ae296b7b6d)

把授权码填写到配置文件`SMTP`下的**SENDER_PASSWORD**的引号里面

# docker运行

> 容器内部的`/app`目录是工作目录，可以把它映射到配置文件所在的目录，运行后会创建`.fav`文件夹，里面有数据库文件，记录了已经下载的视频，如果想全部视频重新下载删除该`.fav`文件夹即可

```bash
# 创建容器并运行，自行修改相关参数
docker run -it --restart=always --name bili-sync-fav -v <你希望存储程序配置的目录>:/app -v <视频想保存的路径>:<配置文件写的收藏夹路径> bili-sync-fav
```

## Compose运行

目前只有 Linux/amd64 平台可使用 Docker 或 Docker Compose 运行，其他平台请[自行编译](#Dockerfile编译运行)，此处以 Compose 为例：

```yml
services:
  bili-sync-fav:
    image: cap153/bili-sync-fav:latest
    restart: unless-stopped
    network_mode: bridge
    hostname: bili-sync-fav
    container_name: bili-sync-fav
    volumes:
      - ${你希望存储程序配置的目录}:/app
      # 还需要有视频下载位置
      # 这些目录不是固定的，只需要确保此处的挂载与 bili-sync-fav 的配置文件相匹配
      - ${视频想保存的路径}:${配置文件写的收藏夹路径}
```

## Dockerfile编译运行

```bash
# 下载最新源码
git clone --depth 1 https://github.com/cap153/bili-sync-fav
# 进入项目目录
cd bili-sync-fav
# 构建docker镜像
docker build -t bili-sync-fav ./
# 创建容器并运行，自行修改相关参数
docker run -it --restart=always --name bili-sync-fav -v <你希望存储程序配置的目录>:/app -v <视频想保存的路径>:<配置文件写的收藏夹路径> bili-sync-fav
```

# 主机运行

## 源码运行

```bash
# 下载最新源码
git clone --depth 1 https://github.com/cap153/bili-sync-fav
# 进入项目目录
cd bili-sync-fav
# 运行代码会在当前目录读取config.toml，请提前配置好
RUST_LOG="bili_sync_fav=info,warn" cargo run --release
```

## 源码编译运行

我在[release](https://github.com/cap153/bili-sync-fav/releases)上传了我编译的可执行文件`bili-sync-fav`(archlinux,amd64,gun)，可以直接下载该文件使用，如果无法正常运行，可以尝试下面的步骤手动编译

```bash
# 下载最新源码
git clone --depth 1 https://github.com/cap153/bili-sync-fav
# 进入项目目录
cd bili-sync-fav
# 编译代码
cargo build --release
```

编译成功后可以在当前目录下的`target/release`里面找到可执行文件`bili-sync-fav`，可以把它复制或移动到`/usr/local/bin`

```bash
# 安装到环境变量
cp target/release/bili-sync-fav /usr/local/bin
# 配置日志等级以显示详细信息
export RUST_LOG="bili_sync_fav=info,warn" 
# 此时该文件可以在任意路径运行，-c参数可以指定配置文件
bili-sycn-fav -c <配置文件>
```
