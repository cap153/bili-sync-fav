# 配置文件

## Cookie信息

当前版本的默认示例文件如下：

```toml
interval = 1200

[credential]
sessdata = ""
bili_jct = ""
buvid3 = ""
dedeuserid = ""
ac_time_value = ""

[favorite_list]
<收藏夹id> = "<保存的路径>"
<收藏夹id> = "<保存的路径>"
```

- `interval`:表示程序每次执行扫描下载的间隔时间，单位为秒。
- `credential`:哔哩哔哩账号的身份凭据，请参考凭据获取[流程获取](https://nemo2011.github.io/bilibili-api/#/get-credential)并对应填写至配置文件中，后续 bili-sync 会在必要时自动刷新身份凭据，不再需要手动管理。推荐使用匿名窗口获取，避免潜在的冲突。
- `favorite_list`:你想要下载的收藏夹与想要保存的位置。简单示例：

```bash
3115878158 = "/home/amtoaer/Downloads/bili-sync/测试收藏夹"
```

## 收藏夹id获取方法

[什么值得买的文章有详细介绍](https://post.smzdm.com/p/a4xl63gk/)，打开收藏夹，点击收藏夹封面

![image](https://github.com/user-attachments/assets/02efefe9-0a3a-46d6-8646-a6aa462d62c2)

浏览器可以看到“mlxxxxxxx”，只需要后面数字即可（不需要“ml“）

![image](https://github.com/user-attachments/assets/270c7f2f-b1b1-49a1-a450-a133f0d459fa)

# docker运行

> 容器内部的`/app`目录是工作目录，可以把它映射到配置文件的路径，运行后会创建`.fav`文件夹，里面有数据库文件，记录了已经下载的视频，如果想全部视频重新下载删除该`.fav`文件夹即可

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
      - ${视频下载的位置}:${配置文件中的路径}
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
docker run -it --restart=always --name bili-sync-fav  -v <配置文件路径>:/app -v <视频想保存的路径>:<配置文件写的收藏夹路径> bili-sync-fav
```

# 主机运行

## 源码运行

```bash
# 克隆仓库
git clone --depth 1 https://github.com/cap153/bili-sync-fav
# 进入项目目录
cd bili-sync-fav
# 运行代码会在当前目录读取config.toml，请提前配置好
RUST_LOG="bili_sync_fav=info,warn" cargo run --release
```

## 源码编译运行

```bash
# 克隆仓库
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
# 此时可以在任意路径运行，-c参数可以指定配置文件路径
bili-sycn-fav -c <配置文件路径>
```

