# SignPath Foundation 申请材料

> 目标：为 GamePeek 的 Windows 可执行文件申请免费代码签名，消除 Edge / SmartScreen 的“未知发布者”拦截。
> 官方入口：<https://signpath.org/apply>，表格填写后发送至 `oss-support@signpath.org`。

## 1. 资格自查

| 条件 | 现状 | 说明 |
|---|---|---|
| 使用 OSI 认可的开源许可证 | ✅ MIT | 仓库根目录 `LICENSE`，`Cargo.toml` 中 `license = "MIT"` |
| 仓库公开可访问 | ✅ | https://gitee.com/junk666/gamepeek |
| 不含专有代码 | ✅ | 全部源码在仓库内，无闭源组件、无内嵌商业 SDK |
| 不含恶意行为 | ✅ | 不注入进程、不读写游戏内存、不联网 |
| 构建过程自动化 | ✅ | `.github/workflows/ci.yml`、`release.yml` |
| 使用受支持的代码托管平台 | ❌ 待办 | SignPath 支持 GitHub / GitLab / Bitbucket / Azure DevOps，**不支持 Gitee**，需镜像到 GitHub |
| 项目持续维护 | ✅ | 2026-11 起持续提交，已有两个 Release |

结论：只差 GitHub 镜像这一步。

## 2. 先把仓库镜像到 GitHub

1. 在 GitHub 新建一个**公开空仓库**（不要勾选 README / .gitignore / license），假设地址为
   `https://github.com/Uing2/gamepeek`。
2. 本地添加远端并推送（在 `GamePeek` 目录执行）：

```powershell
git remote add github https://github.com/Uing2/gamepeek.git
git push github main --tags
```

3. 之后每次发版，Gitee 和 GitHub 各推一次：

```powershell
git push origin main --tags
git push github main --tags
```

> 也可以后续在 Gitee「管理 → 仓库镜像管理 → 推送镜像」里配置自动同步，需要 GitHub 的 Personal Access Token。

推完后确认 GitHub 上能看到：
- `LICENSE`（MIT）
- `.github/workflows/ci.yml` 与 `release.yml`
- Release 页面里的 tag

## 3. 申请邮件（英文，直接复制发送）

收件人：`oss-support@signpath.org`
主题：

```
Application for free code signing: GamePeek (Windows, MIT)
```

正文：

```
Hello SignPath Foundation team,

I would like to apply for free code signing for an open-source Windows utility.

Project name: GamePeek
Repository (public): https://gitee.com/junk666/gamepeek
Mirror (public): https://github.com/Uing2/gamepeek
License: MIT (OSI-approved), see LICENSE in the repository root
Maintainer: <your name>, <your email>

What it does:
GamePeek is a small Windows tray utility for competitive gamers. While playing
Counter-Strike 2, pressing a global hotkey brings a short-video window (Douyin)
above the game and mutes the game audio; pressing the hotkey again (or clicking
back into the game) pauses the video and restores the game audio. It only uses
public Win32 and WASAPI APIs: window positioning, per-process audio mute, and the
Windows media session API. It does not inject code into other processes, does not
hook graphics APIs, does not read or write game memory, and makes no network
requests.

Why we need signing:
Users download a single portable gamepeek.exe from the release page. Because the
binary is currently unsigned, Edge and Microsoft Defender SmartScreen block or
warn on every download, and many users abandon the installation.

Artifacts to sign:
One file: gamepeek.exe (x86_64 Windows executable, ~900 KB, no installer).

Build and release process:
- Language: Rust, target x86_64-pc-windows-msvc
- CI: GitHub Actions
  - .github/workflows/ci.yml: cargo check, cargo test, release build on every push
  - .github/workflows/release.yml: triggered by v* tags; builds, submits the
    artifact for signing, verifies nothing else is added, and publishes the
    signed binary with a SHA256 checksum to the GitHub release
- The workflow already contains the SignPath submission step
  (signpath/github-action-submit-signing-request@v1) and falls back to an
  unsigned artifact when no token is configured.

Signing policy preference:
Single policy for tagged releases, project slug `gamepeek`,
policy slug `release-signing`.

We would be happy to provide any additional information.

Best regards,
<your name>
```

要点：
- 必须用英文；
- 三个链接（仓库、镜像、许可证）一旦填错会被退回；
- 说明“为什么需要签名”时，强调**用户下载被拦截**比强调“看起来专业”更有效；
- 明确只有**一个产物**，审核会更快。

## 4. 审核通过后要配置的东西

SignPath 控制台会给出：

| 项 | 用途 | 放在哪里 |
|---|---|---|
| Organization ID | 标识你的组织 | GitHub Secret `SIGNPATH_ORGANIZATION_ID` |
| API Token | 提交签名请求的凭证 | GitHub Secret `SIGNPATH_API_TOKEN` |
| Project slug | 一般就是 `gamepeek` | GitHub Variable `SIGNPATH_PROJECT_SLUG` |
| Signing policy slug | 例如 `release-signing` | GitHub Variable `SIGNPATH_POLICY_SLUG` |

配置位置：GitHub 仓库 → Settings → Secrets and variables → Actions。

`release.yml` 已经按这四个键名取值，不需要改代码。

## 5. 验证签名

发布后下载 exe，在装了 Windows SDK 的机器上执行：

```powershell
signtool verify /pa /v gamepeek.exe
```

应看到 `Successfully verified`，且签名者的组织信息为 SignPath Foundation。

也可以直接看文件属性 → 数字签名，或在 PowerShell 中：

```powershell
Get-AuthenticodeSignature .\gamepeek.exe | Format-List *
```

`Status` 应为 `Valid`，`SignerCertificate.Subject` 包含 SignPath。

## 6. 注意事项

- **只有 CI 构建的产物会被签名**。本地 `cargo build` 出的 exe 仍然未签名，这是 SignPath 的硬性要求：二进制必须可追溯到仓库源码。
- 签名只解决“发布者身份可信 + 未被篡改”，不再触发 SmartScreen 的“未知发布者”；新证书在部分机器上仍可能有短时间信誉积累过程。
- 如果后续增加安装包（Inno Setup），建议对安装包也签名，并且**先签内部 exe 再打包**，否则安装后仍会提示。
- 免费额度要求项目保持开源；转闭源需要改付费方案或自购证书。
- 签名私钥在 SignPath 的 HSM 中，GitHub 上只需要放 API Token（有权限限制，可随时吊销）。

## 7. 之后可以顺手做的

- README 加上「签名状态」说明，替换掉现在“未签名属正常”的段落；
- Release 附件同时提供 `checksums.txt`（`release.yml` 已自动生成）；
- 在 Gitee 镜像配置里开启自动同步，避免两边版本不一致。