fn main() {
    println!("cargo:rerun-if-changed=assets/app.ico");
    println!("cargo:rerun-if-changed=app.rc");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let out_dir = match std::env::var("OUT_DIR") {
        Ok(dir) => std::path::PathBuf::from(dir),
        Err(_) => return,
    };
    let resource = out_dir.join("gamepeek.res");
    let resource_arg = resource.to_string_lossy().to_string();

    let env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let mut status = None;

    if env == "msvc" {
        status = std::process::Command::new("rc")
            .args(["/nologo", "/fo", &resource_arg, "app.rc"])
            .status()
            .ok();
    }

    if status.map(|s| !s.success()).unwrap_or(true) {
        status = std::process::Command::new("windres")
            .args(["-i", "app.rc", "-o", &resource_arg])
            .status()
            .ok();
    }

    match status {
        Some(s) if s.success() => {
            println!("cargo:rustc-link-arg-bins={resource_arg}");
        }
        _ => {
            // 本机没有 rc/windres 时跳过 exe 图标，托盘与窗口图标不受影响。
            println!("cargo:warning=未找到资源编译器，本次构建不嵌入 exe 文件图标");
        }
    }
}