# GamePeek

打游戏时按一个键，把抖音窗口叠到游戏上层并静音游戏；再按一次（或直接点回游戏）立刻回到游戏，抖音自动暂停。不注入、不 Hook、不联网。

```
GAME 态                          PEEK 态
┌───────────────┐   Alt+C     ┌───────────────┐
│               │  ───────▶   │  CS2（静音）   │
│     CS2       │             │  ┌─────────┐  │
│               │  ◀───────   │  │ 抖音    │  │
└───────────────┘ Alt+C/点CS  │  └─────────┘  │
                              └───────────────┘
```

## 特性

- **单热键切换**：默认 `Alt+C`，可配置。
- **两种返回方式**：再按热键，或直接点击游戏窗口自动返回。
- **按进程静音**：用 WASAPI 精确静音 `cs2.exe`，不影响抖音声音；退出时还原。
- **可靠的暂停**：前台发空格 → 失焦时走 Windows 媒体会话 → 兜底投递按键消息，三层降级。
- **轻量常驻**：Rust + 原生 Win32，无运行时、无 WebView，单文件约 750 KB。
- **异常可恢复**：进程崩溃或被强杀后，下次启动能识别并还原遗留的静音状态。
- **安全边界**：不注入游戏、不 Hook 图形接口、不读写游戏内存、不联网。

## 系统要求

- Windows 10 1809+ / Windows 11（推荐 Win11，媒体会话支持更完整）
- 游戏需使用**全屏窗口化（无边框）**，独占全屏会把任何窗口挡在下面，这是系统行为，无法绕过

## 使用

1. 从 [Releases](https://gitee.com/junk666/gamepeek/releases) 下载已构建的 `gamepeek.exe`，或按下文自行编译，放到任意目录运行。
2. 托盘出现图标后即可使用：
   - 左键单击或按 `Alt+C`：切换
   - 右键：切换、设置、打开配置、打开日志目录、退出
3. 首次运行会在 `%LOCALAPPDATA%\GamePeek\config.toml` 生成配置。

设置界面可以调整热键（支持按键录制）、游戏进程、目标进程与标题关键字、显示位置与尺寸、返回时的隐藏方式、暂停方式，以及自动返回和开机自启。保存后立即生效，不需要重启。也可以用 `gamepeek.exe --settings` 直接打开设置。

## 配置

大部分项可以直接在设置界面改，下面是对应的配置文件内容，手改后重启或从设置界面保存一次即可生效。

```toml
hotkey = "Alt+C"

[game]
process = "cs2.exe"
mute_on_peek = true

[target]
process = "douyin.exe"      # 抖音桌面版进程名，按实际情况修改
window_title = "抖音"        # 进程名匹配不到时按标题匹配
auto_launch = false

[layout]
position = "bottom-right"   # bottom-right / bottom-left / top-right / top-left
width = 480
height = 270
hide_mode = "offscreen"     # offscreen（移出屏幕，切换更快） / minimize

[pause]
method = "auto"             # auto / space / smtc / none
verify_timeout_ms = 100

[auto_return]
on_game_focus = true        # 游戏重新成为前台时自动返回
scope = "game"              # game：仅游戏；any：目标窗口失焦即返回

[app]
start_with_windows = false  # 改为 true 后写入当前用户的开机自启
restore_audio_on_exit = true
```

日志在 `%LOCALAPPDATA%\GamePeek\logs\gamepeek.log`，每次切换都会记录各步骤耗时。

## 从源码构建

需要 Rust stable。本仓库不依赖 C++ 工具链，官方 GNU 目标即可编译：

```powershell
rustup default stable-x86_64-pc-windows-gnu
cargo build --release
```

> 如果项目路径包含中文，MinGW 链接器会报 `cannot open output file`，把输出目录指到纯 ASCII 路径即可：
>
> ```powershell
> $env:CARGO_TARGET_DIR = "$env:LOCALAPPDATA\gamepeek-target"
> cargo build --release
> ```

产物在 `target/release/gamepeek.exe`（或上面 `CARGO_TARGET_DIR` 下的 `release` 目录）。

## 文档

- [项目文档](PROJECT.md)：背景、调研、交互流程、边界情况、验收标准
- [开发手册](DEVELOPMENT.md)：架构、模块实现、调试工具、测试方案

## 已知限制

- 只处理首版目标：CS2 + 抖音桌面客户端，其他组合需要改配置，尚未做 profile 管理。
- 音频设备切换后没有监听设备事件，会在下次切换时重新枚举会话。
- 抖音版本变化可能导致窗口定位失效，可按进程名、窗口类名、标题多条件兜底。
- 若 Steam / 游戏以管理员权限运行，本工具也需要以管理员权限启动，否则无法获取窗口与音频会话。

下载时如果 Edge 或 SmartScreen 提示风险，是因为可执行文件尚未做代码签名，属于正常现象：

- Edge 下载列表 → 该文件右侧 `…` → 保留
- 或下载后右键文件 → 属性 → 勾选“解除锁定”
- 每次 Release 都会附带 `checksums.txt`，可用 `Get-FileHash gamepeek.exe -Algorithm SHA256` 核对

## 免责声明

本工具仅使用公开的 Win32 与 WASAPI 接口做窗口和音频管理，不注入游戏进程。在 CS2 中使用时请遵守游戏与平台规则；因使用本工具产生的任何后果由使用者自行承担。

## License

[MIT](LICENSE)