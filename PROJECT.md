# GamePeek 项目文档

> 打游戏时一键把抖音窗口叠到游戏上层、静音游戏；再按一次回到游戏并暂停抖音。工作名 GamePeek，可随时更换。

| 项目 | 内容 |
|---|---|
| 文档版本 | v0.1（草案） |
| 日期 | 2026-11-19 |
| 状态 | M1 核心闭环已实现，待实机验证 |
| 平台 | Windows 10 1809+ / Windows 11 |
| 首版范围 | CS2 + 抖音 Windows 桌面客户端 |
| 技术栈 | Rust 1.100（stable-x86_64-pc-windows-gnu），原生 Win32/WASAPI |
| 配套文档 | [开发手册](./DEVELOPMENT.md) |

---

## 1. 背景与问题

打 CS 时想顺带刷抖音，现有做法都有明显摩擦：

- Alt-Tab 切到抖音：游戏失去焦点，听不到游戏声音，切回来还要再找窗口。
- 把抖音置顶：需要额外工具，且游戏声音和抖音声音会混在一起。
- 用手机看：视线离开屏幕，体验割裂。

目标是把这套动作压缩成**一个热键、两个状态、零等待**：按一下进入"抖音叠在游戏上"的状态，再按一下回到游戏，抖音自动暂停。

## 2. 目标与非目标

### 2.1 目标

1. 单热键在"游戏态"和"瞥屏态"之间切换，切换过程无可感知卡顿。
2. 瞥屏态：抖音窗口可见、置顶、可操作（滚轮/方向键刷视频），游戏音频静音。
3. 返回游戏态：抖音暂停并退到后台，恢复游戏音频，焦点回到游戏。
4. 手动切回游戏同样生效：用户点击 CS 窗口、Alt-Tab 或点任务栏让 CS 重新获得前台时，自动执行返回流程。
5. 常驻托盘、开机自启，热键响应不受后台节流影响。
6. 架构上为"任意游戏 + 任意视频应用"预留扩展点，但首版只做 CS2 + 抖音。

### 2.2 非目标

- 不做 DLL 注入、画面 hook、游戏内 overlay。只使用 Win32 窗口管理和 WASAPI 音频控制。
- 不把抖音嵌入游戏画面，不修改游戏渲染。
- 不做视频下载、推荐流接管、账号相关功能。
- 首版不做多套配置的图形化管理器（配置文件 + 最小设置页即可）。

## 3. 现有方案调研结论

按 GitHub、中文社区和现成工具的检索结果，**没有找到完全实现该功能的开源项目**，但每个环节都有成熟实现可以复用。

| 方案 | 已具备能力 | 与本需求的差距 |
|---|---|---|
| PowerToys Always on Top / DeskPins / WindowTop | 任意窗口热键置顶 | 不控制音频、不切焦点、不控制播放；PowerToys 在 Game Mode 下默认不触发 |
| BossKey 类工具（含开源实现） | 一键隐藏/显示窗口、静音窗口、暂停视频 | 逻辑是"藏起来"，不是"叠到游戏上"，也不恢复游戏焦点 |
| OneQuick（旧版开源 AHK） | 热键多步动作、窗口置顶、模拟按键 | 没有按进程静音，动作链需自行拼装 |
| AMM TriggerKit（商店闭源） | 依据 CS2 事件自动暂停/压低音乐，走 Windows 媒体会话 | 不显示抖音、不静音游戏，闭源 |
| Audio.ahk / MuteActiveWindow | 按窗口或进程静音、发送媒体键 | 脚本片段，无产品化封装 |

结论：需求成立，且实现路径清晰，可以自研。首版的价值不在"技术难度"，而在"把散件拼成稳定、无感的产品"。

## 4. 使用场景与交互流程

### 4.1 状态定义

| 状态 | 前台窗口 | 抖音 | CS2 音频 |
|---|---|---|---|
| GAME（游戏态） | CS2 | 后台、暂停、不置顶 | 正常 |
| PEEK（瞥屏态） | 抖音 | 置顶、指定位置和尺寸、可交互 | 静音 |

### 4.2 热键与状态判定

- 默认热键：`F8`，可配置。备选鼠标侧键，避免与游戏内绑定冲突。
- 使用 `RegisterHotKey` 注册，**不使用全局低级键盘钩子**，避免给每次按键增加拦截层。
- 动作由**当前前台窗口**决定，而不是内部布尔值：
  - 前台是抖音（含其子窗口）→ 切到 GAME。
  - 其他情况 → 切到 PEEK。
  - 这样用户手动 Alt-Tab、点其他窗口后，状态不会错乱。

### 4.3 进入 PEEK

1. 校验抖音窗口句柄；失效则按进程路径 + 窗口类名重新解析；找不到则托盘提示并中止。
2. 快照 CS2 音频会话当前的静音状态。
3. 静音 `cs2.exe` 的全部音频会话。
4. 抖音窗口恢复到记忆的矩形（首次使用为右下角预设尺寸），`ShowWindow(SW_SHOWNOACTIVATE)` + `SetWindowPos(HWND_TOPMOST, SWP_NOACTIVATE)`。
5. `SetForegroundWindow(抖音)`，确认前台切换成功。
6. 可选：若抖音被系统级静音，按配置决定是否取消静音。

### 4.4 返回 GAME

1. 暂停目标，按可靠性降级：
   - 优先用 Windows 媒体会话按音源暂停，不依赖焦点，前后台都能用。
   - 没有媒体会话、且是热键触发并确认抖音仍在前台时，才发送空格；发送前轮询 `GetForegroundWindow() == 抖音 HWND`，超时改用投递消息。
   - 手动点击 CS 时抖音已经失焦，**禁止发空格**（会打到 CS 导致角色跳跃），直接走媒体会话或 `PostMessage`。
2. 校验抖音已暂停；未成功则记录日志并在托盘提示。
3. 取消 `TOPMOST`，将窗口移出屏幕或最小化（可配置）。
4. 还原 CS2 音频会话此前快照的静音状态（只还原本工具改过的会话）。
5. 若当前前台不是 CS2，`SetForegroundWindow(CS2)` 并校验；已经在前台则跳过。
6. 任一关键步骤失败时重试一次，仍失败则托盘提示，不静默失败。

### 4.5 手动切回游戏（自动返回）

- 监听 `EVENT_SYSTEM_FOREGROUND`，当 CS2 成为前台时，自动执行 4.4 的返回流程，不需要再按热键。
- 触发来源包括：点击 CS 窗口的可见区域、Alt-Tab、任务栏、Win+D 等。
- 自身切换产生的前台变化必须抑制：进入 PEEK 时记录期望前台窗口（抖音），切换完成前忽略事件；事件处理做短时间去抖，避免一次切换触发多次返回。
- 默认只在 CS2 获得前台时自动返回；切换到浏览器、Discord 等其他应用不自动暂停，避免误伤。该策略可配置。

### 4.6 异常路径

- 抖音中途被关闭：立即还原 CS2 音频，状态回到 GAME。
- 用户手动 Alt-Tab：下次热键以实际前台窗口为准。
- 工具退出或崩溃：干净退出时还原音频；启动时若发现"上次是我静音的"标记，按配置还原或提示。
- 系统睡眠/唤醒、显示器热插拔后窗口句柄失效：重新解析并刷新缓存。

## 5. 功能需求

### P0（首版必须）

- 全局热键切换，可配置。
- 抖音窗口置顶/取消置顶、显示/隐藏、位置与尺寸应用。
- CS2 按进程静音与状态还原。
- 抖音暂停（前台时空格优先，失焦时媒体会话优先）。
- 前台监听：CS2 重新成为前台时自动执行返回流程，无需再按热键。
- 配置文件读写，缺失时生成默认配置。
- 托盘图标：切换、暂停/恢复、打开配置、退出。
- 单实例运行、开机自启。
- 异常退出后的音频还原。
- 基础日志，便于定位"偶尔不灵"。

### P1（首版尽量）

- 窗口与音频会话预热、缓存、事件驱动刷新。
- 多显示器与 DPI 适配，记忆抖音位置和尺寸。
- 抖音多窗口选择（主窗/直播/小窗）并持久化。
- 设置界面：热键、目标进程、位置预设、隐藏方式。
- 诊断面板：显示当前状态、前台窗口、音频会话、最近一次切换耗时。

### P2（后续）

- 任意游戏 + 任意视频应用的 profile 配置。
- 第二种模式："只挂着看、不抢焦点"。
- 单独控制游戏语音与音效（需游戏内指令配合，风险较高）。
- 多套热键与一键恢复。

## 6. 非功能需求

| 指标 | 目标 |
|---|---|
| 冷启动到托盘可用 | < 150 ms（Rust 原生） |
| 热键回调处理 | P95 < 30 ms |
| 进入 PEEK 的可见切换 | < 100 ms，抖音无黑帧 |
| 返回 GAME | < 100 ms，CS2 可立即操作 |
| 常驻内存 | < 30 MB（不含游戏与抖音） |
| 空闲 CPU | < 0.1% |
| 安全边界 | 不注入、不 hook、不联网、不需要驱动 |

## 7. 技术方案

### 7.1 架构

```
        [RegisterHotKey 消息循环]
                    |
                    v
            [ToggleController] <---- [ForegroundWatcher / WinEventHook]
             /        |         \
            v         v          v
     [WindowManager] [AudioManager] [MediaController(可选)]
            |             |            |
        Win32 API      WASAPI        SMTC / SendInput
                    |
              [Config / Tray / Diagnostics]
```

### 7.2 模块职责

| 模块 | 职责 |
|---|---|
| Hotkey | 注册全局热键，维护消息循环，回调只投递事件不做重活 |
| ForegroundWatcher | 监听前台窗口变化，识别 CS2 重新获得焦点并触发自动返回，抑制自身切换产生的事件 |
| ToggleController | 状态判定与流程编排，保证异常路径可回滚 |
| WindowManager | 窗口查找、置顶、显示、移动、焦点切换 |
| AudioManager | 枚举会话、按 PID 静音、快照与还原、设备变化监听 |
| MediaController | 空格发送、前台校验、媒体会话兜底 |
| Config | 读写配置，默认值与迁移 |
| Tray | 托盘菜单、状态提示、退出路径 |
| Diagnostics | 日志、耗时打点、状态快照 |

### 7.3 关键实现点

**热路径零查找。** 窗口句柄和 `ISimpleAudioVolume` 在启动或进程出现时解析并缓存；用 `SetWinEventHook(EVENT_SYSTEM_FOREGROUND)` 维护前台状态，用音频会话事件使失效缓存重建。热键回调里只做置顶、静音、切焦点三类操作。

**焦点切换。** `SetForegroundWindow` 受前台锁限制，必要时使用 `AttachThreadInput`，或临时把 `SPI_SETFOREGROUNDLOCKTIMEOUT` 置 0，调用后恢复。切换后校验 `GetForegroundWindow()`，避免"按了没反应"。

**避免黑闪。** 抖音不最小化，改为移动到屏幕外或缩到 1×1 保留渲染；显示时只移动不改尺寸。尺寸变化会重建交换链，是黑闪的主要来源。

**空格竞态。** 空格只能发给已确认在前台的抖音窗口，否则会误发给 CS2 导致角色跳跃。发送前轮询前台窗口，带 50–100 ms 超时；失败则改用媒体会话暂停。

**状态以事实为准。** 不维护"当前在 PEEK"的布尔值，始终根据前台窗口判断，避免手动切换后状态漂移。

**自动返回与事件抑制。** 手动点击 CS 时抖音已经失焦，此时不能再用空格（会打到游戏），暂停必须走媒体会话，或向目标窗口 `PostMessage` 补发。监听 `EVENT_SYSTEM_FOREGROUND` 时，用"期望前台窗口 + 短时间去抖"区分用户操作和工具自身的焦点切换，避免进入 PEEK 的瞬间被自己触发返回。

**还原要精确。** 只还原工具改过的音频会话，且保留用户原本的静音状态；记录"本次由我静音"的标记，防止把用户自己静音的会话错误地取消静音。

### 7.4 技术选型

| 方案 | 启动 | 内存 | 开发成本 | 结论 |
|---|---|---|---|---|
| Rust + 原生 Win32 托盘（不用 Tauri） | 几十毫秒 | 约 15 MB | 高，WASAPI COM 需手写 | 首选，符合"启动快、切换稳" |
| C# + WinForms | 0.3–0.6 s | 40–60 MB | 低 | 可接受的折中，达不到极致启动 |
| AutoHotkey v2 | 几乎瞬时 | 10–20 MB | 最低 | 仅用于可行性验证，不做正式版 |

选定 Rust 时，使用 `windows` crate 调 Win32/WASAPI/SMTC，避免引入 WebView2。若后续确认开发成本过高，再退回 C#，功能设计不变。

### 7.5 配置草案

```toml
hotkey = "F8"

[game]
process = "cs2.exe"
# 可选：失焦时是否由本工具负责静音
mute_on_peek = true

[target]
process = "douyin.exe"          # 待实测确认实际进程名
window_class = ""               # 可选，精确匹配
window_title = "抖音"            # 模糊匹配兜底
auto_launch = false

[layout]
position = "bottom-right"       # bottom-right / top-right / 记忆位置
width = 480
height = 270
hide_mode = "offscreen"         # offscreen / minimize

[pause]
method = "auto"                 # auto / space / smtc / none
verify_timeout_ms = 100

[auto_return]
on_game_focus = true            # CS2 重新成为前台时自动返回
scope = "game"                  # game / any

[app]
start_with_windows = true
start_minimized = true
restore_audio_on_exit = true
```

## 8. 边界与特殊情况

| 情况 | 表现 | 处理 |
|---|---|---|
| 权限不匹配 | Steam/CS 以管理员运行，普通权限工具拿不到窗口或音频会话 | 检测进程完整性级别，提示以管理员重启；不默认全程提权 |
| Win11 节能节流 | 后台托盘进程被 EcoQoS 降频，热键偶发延迟 | `SetProcessInformation(ProcessPowerThrottling)` 关闭执行节流 |
| CS2 独占全屏 | 抖音无法显示在游戏之上 | 启动时检测并提示切换为"全屏窗口化"；建议对 `cs2.exe` 关闭全屏优化 |
| CS2 自带失焦静音 | 与工具逻辑重复或互相覆盖 | 启动时读取并提示，建议关闭游戏内选项，统一由工具控制 |
| 抖音自身置顶/画中画 | 清除 `TOPMOST` 后仍停留在最上层 | 返回时直接移出屏幕或最小化，不只清置顶 |
| 抖音多个窗口 | 主窗、直播间、小窗各有句柄 | 首次让用户选择并持久化句柄；失效时按进程路径 + 类名重找 |
| 抖音全屏播放 | 恢复后仍保持全屏，遮挡游戏 | 进入 PEEK 前强制恢复为普通窗口尺寸 |
| 音频设备切换 | 换耳机后缓存的会话失效 | 注册 `IMMNotificationClient`，设备变化后重建音频会话 |
| 多显示器与 DPI | 窗口恢复到错误位置或尺寸 | 保存原始 RECT 和显示器标识，按目标 DPI 换算 |
| 虚拟桌面 | 抖音在另一个虚拟桌面，无法激活 | 检测 `IVirtualDesktopManager`，尝试移动到当前桌面，失败则提示 |
| 资源管理器重启 | 托盘图标消失 | 监听 `TaskbarCreated` 消息并重新添加图标 |
| 中文输入法 | 带字母的热键被输入法吞掉 | 默认使用 F8/F9 或鼠标侧键 |
| 工具崩溃 | 游戏持续静音或窗口停留在错误状态 | 持久化"是我改的"标记，启动时还原；干净退出时同步还原 |
| 睡眠/唤醒 | 窗口句柄、音频会话失效 | 监听电源事件，唤醒后刷新缓存 |
| 手动点击/切回 CS | 抖音继续播放、CS 仍静音 | 前台监听触发完整返回；抖音已失焦时走媒体会话，禁止发空格 |
| 工具自身切换焦点 | 误判为用户切回，进入 PEEK 后立刻弹回 | 记录期望前台窗口并抑制事件，事件处理加短时去抖 |
| 点击其他应用 | 是否暂停不明确 | 默认只在 CS2 获得前台时返回；可配置为任意失焦都暂停 |
| 抖音无媒体会话且 PostMessage 无效 | 失焦后无法暂停 | M0 验证三种暂停方式，至少一条可用；否则收起窗口并提示 |
| 通用化后的其他游戏 | 失焦时自动最小化，回来找不到窗口 | 记录并恢复窗口显示状态（P2） |

## 9. 风险与待验证项

| 风险 | 影响 | 验证方式 |
|---|---|---|
| 抖音桌面版暂停行为不确定 | 可能无法可靠暂停，或重新聚焦自动续播 | 手动测试：前台空格、失焦空格、媒体键、重聚焦 |
| 抖音失焦后无法暂停 | 点回 CS 后抖音仍在播放，与游戏声音叠加 | 依次验证 SMTC 会话、PostMessage、全局媒体键，至少一条可用 |
| 焦点切换被前台锁拒绝 | 热键需要按两次或切不过去 | 在 CS2 无边框前台反复切换，统计失败率 |
| 抖音显示时黑闪 | 体验不流畅，违背核心目标 | 从屏幕外移入、从最小化恢复各测 20 次 |
| 切换焦点导致 CS2 卡顿 | 影响游戏体验 | 无边框模式下记录帧时间与画面停顿 |
| 抖音进程/窗口结构随版本变化 | 找不到窗口 | 多条件匹配 + 配置项兜底，升级后回归测试 |
| 第三方平台对后台工具的误判 | 理论上无注入风险，但仍需谨慎 | 仅使用公开 Win32/WASAPI 接口，不读写游戏内存 |

## 10. 里程碑

| 阶段 | 内容 | 产出 |
|---|---|---|
| M0 可行性验证 | 验证焦点切换、失焦暂停、显示黑闪三个未知点 | 验证记录与实现方式结论 |
| M1 核心闭环 | 热键、置顶、按进程静音、暂停、前台自动返回、配置文件、托盘 | 可日常使用的单文件程序 |
| M2 稳定性 | 预热缓存、异常还原、设备变化、日志与耗时打点 | 连续使用无状态残留 |
| M3 体验完善 | 设置界面、位置记忆、多显示器/DPI、自启与打包 | 可分发的首个版本 |
| M4 扩展 | 通用 profile、只挂着看模式、语音单独控制 | 后续版本 |

## 11. 验收标准

1. 游戏态按热键，抖音在 100 ms 内可见且可操作，CS2 静音，过程中无可感知黑帧。
2. 再按热键，抖音暂停并退出置顶，CS2 声音恢复，焦点回到游戏，可立即操作。
3. 在 PEEK 态直接点击 CS 可见区域，抖音在 200 ms 内暂停并收起，CS2 声音恢复，无需再按热键。
4. 连续切换 100 次，无状态错乱、无焦点丢失、无窗口位置漂移。
5. 手动 Alt-Tab 打乱状态后，热键仍按实际前台窗口正确动作。
6. 强制结束工具后重新启动，能识别并还原遗留的静音状态。
7. 空闲时 CPU 接近 0，内存低于 30 MB（Rust 方案）。
8. 全程不注入游戏进程，不安装驱动，不联网。

## 12. 待确认问题

1. 正式技术栈选 Rust 原生，还是 C# 快速出活？
2. 默认热键用 F8 还是鼠标侧键？
3. 抖音的预设位置和尺寸：右下角 480×270，还是记忆上次位置？
4. 返回游戏时，抖音是"暂停并移出屏幕"还是"暂停并最小化"？
5. 是否需要"只挂着看、不抢焦点"的第二种模式？
6. 是否需要在进入 PEEK 时自动启动抖音？
7. 项目正式名称。
8. 手动点击 CS 时，是完整返回（收起抖音并恢复游戏声音），还是只暂停、窗口继续留在角落？
9. 点击浏览器、Discord 等其他应用时，抖音是否也暂停？默认只对 CS2 生效。

## 13. 参考资料

- [PowerToys Always on Top](https://learn.microsoft.com/zh-cn/windows/powertoys/always-on-top)
- [BossKey（开源）](https://github.com/brucevon/bosskey)
- [Boss Key 功能介绍](https://soft.china.com/details/54/1543752.html)
- [OneQuick 操作项](https://onequick.org/reference/operation)
- [AMM TriggerKit](https://apps.microsoft.com/detail/9mvtj4r71z97)
- [Audio.ahk（按进程控制音频）](https://github.com/thqby/ahk2_lib/blob/master/Audio.ahk)
- [MuteActiveWindow](https://www.majorgeeks.com/files/details/muteactivewindow.html)
- [CS2 无边框与 overlay 可行性](https://backgrind.com/play/counter-strike-2/)
- [CS2 反作弊与 overlay 风险说明](https://backgrind.com/overlay-anticheat-checker/counter-strike-2/)
- [RegisterHotKey](https://learn.microsoft.com/windows/win32/api/winuser/nf-winuser-registerhotkey)
- [SetWindowPos](https://learn.microsoft.com/windows/win32/api/winuser/nf-winuser-setwindowpos)
- [SetForegroundWindow](https://learn.microsoft.com/windows/win32/api/winuser/nf-winuser-setforegroundwindow)
- [IAudioSessionManager2](https://learn.microsoft.com/windows/win32/api/audiosessiontypes/nn-audiosessiontypes-iaudiosessionmanager2)
- [ISimpleAudioVolume](https://learn.microsoft.com/windows/win32/api/endpointvolume/nn-endpointvolume-isimpleaudiovolume)
- [SetProcessInformation（关闭节能节流）](https://learn.microsoft.com/windows/win32/api/processthreadsapi/nf-processthreadsapi-setprocessinformation)
- [GlobalSystemMediaTransportControlsSessionManager](https://learn.microsoft.com/uwp/api/windows.media.control.globalsystemmediatransportcontrolssessionmanager)
