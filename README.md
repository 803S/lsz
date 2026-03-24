# lsz

> 给文件写注释、给目录做收藏、快速了解项目内容的终端文件助手。

`lsz` 是一个面向日常开发和目录浏览的命令行工具。  
它保留了 `ls` 的轻快，又补上了三个真正有用的能力：

- 给文件和目录写注释
- 给常用目录加收藏
- 在终端里快速看 README、代码、压缩包和项目卡片

数据默认保存在 `~/.lsz.db`。

## 核心能力

- 持久化注释：给难记的文件、目录补一句人话说明
- 目录收藏：给常用路径起名字，后面直接取用
- 轻量列表：默认就能看到文件名、注释和摘要
- 项目卡片：`-i` 优先看 README、AGENTS、GUIDE 这类说明文档
- TUI 预览：`-l` 里搜索、过滤、阅读、横向滚动、切换行号
- 代码高亮：支持常见代码、脚本、配置文件
- 压缩包树形预览：快速看 archive 结构，不必先解压

## 构建

```bash
cargo build
```

## 安装

源码安装：

```bash
cargo install --path .
```
~~可直接将可执行文件放到$PATH目录下~~

调试运行：

```bash
./target/debug/lsz -h
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
