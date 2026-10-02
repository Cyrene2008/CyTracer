use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use crate::error::{CoreError, Result};

/// 搜索 ffmpeg / ffprobe 可执行文件所在目录。
///
/// 顺序：
/// 1. 环境变量 `CYTRACER_FFMPEG_DIR`
/// 2. 可执行文件同级 `ffmpeg/bin`
/// 3. 可执行文件同级目录
/// 4. 开发态：仓库内 `src-tauri/ffmpeg/bin`
pub fn search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(dir) = std::env::var("CYTRACER_FFMPEG_DIR") {
        if !dir.is_empty() {
            dirs.push(PathBuf::from(dir));
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            dirs.push(parent.join("ffmpeg").join("bin"));
            dirs.push(parent.to_path_buf());
            dirs.push(parent.join("resources").join("ffmpeg").join("bin"));
        }
    }
    dirs.push(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("src-tauri")
            .join("ffmpeg")
            .join("bin"),
    );
    dirs
}

/// 定位可执行文件；`name` 不带扩展名，如 "ffmpeg"。
pub fn locate(name: &str) -> Result<PathBuf> {
    let file = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    for dir in search_dirs() {
        let candidate = dir.join(&file);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    // 回退 PATH
    let candidate = PathBuf::from(&file);
    if which(&file) {
        return Ok(candidate);
    }
    Err(CoreError::FfmpegNotFound(name.to_string()))
}

fn which(file: &str) -> bool {
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            if dir.join(file).is_file() {
                return true;
            }
        }
    }
    false
}

/// 构造无窗口闪出的子进程命令。
pub fn command(program: &Path) -> Command {
    let mut cmd = Command::new(program);
    cmd.stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// 运行并捕获输出（用于 ffprobe / ffmpeg -version 等短命令）。
pub fn run_capture(program: &Path, args: &[&str]) -> Result<Output> {
    let output = command(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;
    Ok(output)
}

/// 运行并要求成功，返回 stdout 文本；失败时携带 stderr 尾部信息。
pub fn run_ok(program: &Path, args: &[&str]) -> Result<String> {
    let output = run_capture(program, args)?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let tail: String = stderr.chars().rev().take(600).collect::<String>().chars().rev().collect();
        Err(CoreError::FfmpegFailed(tail.trim().to_string()))
    }
}

/// 读取 ffmpeg -version 首行，用于状态展示。
pub fn ffmpeg_version() -> Option<String> {
    let ffmpeg = locate("ffmpeg").ok()?;
    let out = run_capture(&ffmpeg, &["-version"]).ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines().next().map(|line| line.trim().to_string())
}
