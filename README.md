# lsz

> 给文件写注释、给目录做收藏、快速了解项目内容的终端文件助手。

`lsz` 是一个面向日常开发和目录浏览的命令行工具。  
它保留了 `ls` 的轻快，又补上了三个真正有用的能力：

- 给文件和目录写注释
- 给常用目录加收藏
- 在终端里快速看 README、代码、压缩包和项目卡片

数据默认保存在 `~/.lsz.db`。

---
<img width="500" height="400" alt="image" src="https://github.com/user-attachments/assets/828716cd-7e72-413b-9620-569801c8202d" />
<img width="500" height="400" alt="image" src="https://github.com/user-attachments/assets/58d67641-1c95-40f9-926e-c61c26939b7e" />

<img width="500" height="400" alt="image" src="https://github.com/user-attachments/assets/0103b4cb-e9ae-4253-9b85-32a69f26073c" />
<img width="500" height="400" alt="image" src="https://github.com/user-attachments/assets/253a8503-05e9-43a9-847b-07acaea8a5b0" />

---

## 核心能力

- 持久化注释：给难记的文件、目录补一句人话说明
- 目录收藏：给常用路径起名字，后面直接取用
- 轻量列表：默认就能看到文件名、注释和摘要
- 项目卡片：`-i` 优先看 README、AGENTS、GUIDE 这类说明文档
- TUI 预览：`-l` 里搜索、过滤、阅读、横向滚动、切换行号
- 代码高亮：支持常见代码、脚本、配置文件
- 压缩包树形预览：快速看 archive 结构，不必先解压

## 安装

源码安装：

```bash
# cargo install --path .
cd /tmp && git clone https://github.com/803S/lsz.git && cd ./lsz && cargo install --path .
```
无编译环境，linux_x64：

```bash
wget https://github.com/803S/lsz/releases/download/lsz/lsz && sudo mv ./lsz /usr/local/bin/lsz && sudo chmod 755 /usr/local/bin/lsz && echo "ok!"
```

## 常用命令

基础查看：

```bash
lsz [path]            # 轻量列表
lsz --plain [path]    # 纯文本列表，适合管道
lsz -i [path]         # 项目卡片 / 说明文档预览
lsz -l [path]         # TUI 浏览、搜索、阅读
```

注释：

```bash
lsz -s "这是核心目录" ./src
lsz -d ./src
lsz -gc
```

收藏：

```bash
lsz -b add work ~/work/project
lsz -b work
lsz -b list
lsz -b del work
```

## TUI 常用操作

```text
/            搜索
Enter        应用搜索 / 打开目录 / 进入阅读
Esc          清除当前过滤
j / k        上下移动
Tab          切换焦点
:            打开命令行
n            切换代码行号
o            交给系统打开
? / F1       打开帮助
q            退出
```

搜索说明：

- 输入 `/` 后直接过滤名称、path、注释、摘要
- `Enter` 应用过滤并回到正常操作
- `Esc` 清除当前过滤
- 再按 `/` 会保留当前关键字，方便继续改

## 数据存储

```text
~/.lsz.db
```

- 注释和收藏都保存在这个 SQLite 文件里
- 删除文件后可用 `lsz -gc` 清理失效注释记录

## 测试

```bash
cargo test
```
