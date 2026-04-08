# WinMac

> 一个用 Rust 写的轻量级 Windows 托盘小工具。  
> 目标很直接: 把一部分更像 macOS 的窗口快捷键体验，搬到 Windows 上。

![WinMac Icon](assets/winmac-preview.png)

## 软萌介绍

`WinMac` 是一个原生 Win32 + Rust 项目，没有 Electron、没有 WebView、没有大体积运行时。

它主要做两件事:

- `Alt + W` 最小化当前活动窗口
- `Alt + Q` 双击关闭当前活动窗口

同时它还会常驻系统托盘，支持:

- 关闭主窗口时缩到托盘
- 托盘右键菜单控制显示/隐藏
- 开机自启动开关
- 全屏应用下忽略快捷键
- 退出提示浮层

## 功能一览

### 1. `Alt + W`

- 最小化当前活动窗口
- 如果刚刚最小化后你没有切到新的应用，第二次按下可以把刚才那个窗口还原
- 如果你已经点击了新的应用，后续 `Alt + W` 会作用在新的活动窗口上
- 全屏应用下不会响应

### 2. `Alt + Q`

- 第一次按下会弹出一个确认浮层
- 在短时间内再次按下，关闭当前活动窗口
- 全屏应用下不会响应

### 3. 托盘行为

- 点主窗口右上角关闭按钮时，不退出，只隐藏到托盘
- 托盘右键菜单可控制:
- 显示/隐藏设置窗口
- 恢复最近隐藏窗口
- 恢复全部隐藏窗口
- 切换开机自启动
- 退出程序

## 预览

### 图标

![Icon Preview](assets/winmac-preview.png)

### 退出提示浮层

![Overlay Preview](assets/overlay-closeup-smoketest.png)

## 技术栈

- Rust
- 原生 Win32 API
- Direct2D / DirectWrite
- DWM

## 设计目标

- 轻量
- 原生
- 常驻托盘
- 尽量接近 mac 风格的快捷键手感
- 不碰浏览器壳

## 运行方式

### 开发运行

```powershell
cargo run --release
```

### 构建发布版

```powershell
cargo build --release
```

生成的可执行文件在:

```text
target/release/winmac.exe
```

## 项目结构

```text
.
├─ assets/                图标与预览图
├─ src/
│  ├─ main.rs             主程序、托盘、热键、窗口管理
│  └─ overlay_renderer.rs 浮层渲染
├─ build.rs               Windows 资源编译
├─ Cargo.toml
└─ README.md
```

## 当前规则

- 仅支持 Windows
- 对管理员权限窗口、受保护窗口、某些系统窗口，普通权限下可能无法操作
- 全屏应用下，`Alt + W` 和 `Alt + Q` 都会被忽略

## 为什么它看起来有点二次元

因为一个小工具也可以有自己的气质。  
不想做成硬邦邦的系统样式，所以 README 也稍微保留一点卡通感，但代码本体依然偏工程化。

## 小小备注

如果你也喜欢这种“Windows 上补一点 mac 习惯”的方向，欢迎直接拿去改、继续做、换皮肤、加更多快捷键。
