use std::{
    collections::HashSet,
    fs::{self, File},
    io::{self, Read, Seek, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail, ensure};
use moye_plugin_api::{
    Call, Download, Event, Listing, MAX_PAGE_SIZE, Manifest, PROTOCOL_VERSION, Request,
};
use serde_json::Value;
use tempfile::TempDir;

const MAX_MANIFEST: u64 = 32 * 1024;
const MAX_EVENTS: u64 = 8 * 1024 * 1024;
const OUTPUT_FOLDER: &str = "books";

#[derive(Clone)]
pub(crate) struct Plugin {
    pub name: String,
    pub id: String,
    executable: PathBuf,
    data_dir: PathBuf,
    capabilities: Vec<String>,
}

impl Plugin {
    pub fn supports(&self, capability: &str) -> bool {
        self.capabilities.iter().any(|item| item == capability)
    }

    pub fn is_book_source(&self) -> bool {
        self.supports("source.list") && self.supports("source.fetch")
    }

    pub fn directory(&self) -> &Path {
        self.executable.parent().expect("validated plugin path")
    }

    pub fn has_directory_settings(&self) -> bool {
        self.supports("settings.directory.get") && self.supports("settings.directory.set")
    }

    pub fn has_settings(&self) -> bool {
        self.has_directory_settings()
            || self.supports("source.configure")
            || self.supports("source.disconnect")
    }
}

#[derive(Default)]
pub(crate) struct Catalog {
    pub plugins: Vec<Plugin>,
    pub errors: Vec<String>,
}

pub(crate) fn discover(root: &Path) -> Result<Catalog> {
    let dir = root.join("plugins");
    let folders = match fs::read_dir(&dir) {
        Ok(folders) => folders,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Catalog::default()),
        Err(error) => return Err(error.into()),
    };
    let mut catalog = Catalog::default();
    let mut folders = folders.collect::<io::Result<Vec<_>>>()?;
    folders.sort_by_key(|folder| folder.file_name());
    let mut ids = HashSet::new();
    let canonical_dir = dir.canonicalize()?;
    for folder in folders {
        if !folder.file_type()?.is_dir() || !folder.path().join("plugin.json").is_file() {
            continue;
        }
        let result = load_plugin(&canonical_dir, &folder.path(), root).and_then(|plugin| {
            ensure!(ids.insert(plugin.id.clone()), "插件标识重复");
            Ok(plugin)
        });
        match result {
            Ok(plugin) => catalog.plugins.push(plugin),
            Err(error) => catalog.errors.push(format!(
                "{}：{error:#}",
                folder.file_name().to_string_lossy()
            )),
        }
    }
    catalog
        .plugins
        .sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
    Ok(catalog)
}

fn load_plugin(dir: &Path, package: &Path, root: &Path) -> Result<Plugin> {
    let package = package.canonicalize()?;
    ensure!(package.parent() == Some(dir), "插件目录不在安装位置内");
    let mut bytes = Vec::new();
    let manifest_path = package.join("plugin.json");
    ensure!(
        fs::symlink_metadata(&manifest_path)?.is_file(),
        "插件清单必须是普通文件"
    );
    File::open(manifest_path)?
        .take(MAX_MANIFEST + 1)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() as u64 <= MAX_MANIFEST, "插件清单太大");
    let manifest: Manifest = serde_json::from_slice(&bytes).context("无法读取插件清单")?;
    ensure!(
        manifest.protocol == PROTOCOL_VERSION,
        "需要插件协议版本 {}，当前墨页支持 {}",
        manifest.protocol,
        PROTOCOL_VERSION
    );
    ensure!(valid_token(&manifest.id), "插件标识无效");
    ensure!(valid_token(&manifest.executable), "插件可执行文件名称无效");
    ensure!(
        !manifest.name.trim().is_empty()
            && manifest.name.chars().count() <= 60
            && !manifest.name.chars().any(char::is_control),
        "插件名称无效"
    );
    let mut executable = package.join(&manifest.executable);
    if cfg!(target_os = "windows") {
        executable.set_extension("exe");
    }
    let executable = executable.canonicalize().context("找不到插件程序")?;
    ensure!(
        executable.parent() == Some(package.as_path()) && executable.is_file(),
        "插件程序必须位于安装目录内"
    );
    let data_dir = root.join("plugin-data").join(&manifest.id);
    Ok(Plugin {
        id: manifest.id,
        name: manifest.name.trim().to_owned(),
        executable,
        data_dir,
        capabilities: manifest.capabilities,
    })
}

fn valid_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 80
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
}

pub(crate) struct Running {
    child: Child,
    events: File,
    buffer: Vec<u8>,
    read: u64,
    result: Option<Value>,
    progress: Option<String>,
    scratch: Option<TempDir>,
    started: Instant,
    timeout: Duration,
}

impl Running {
    pub fn start(plugin: Plugin, call: Call) -> Result<Self> {
        ensure!(plugin.supports(call.capability()), "插件未声明此功能");
        let timeout = Duration::from_secs(if matches!(call, Call::Fetch { .. }) {
            900
        } else {
            180
        });
        fs::create_dir_all(&plugin.data_dir)?;
        let scratch = tempfile::Builder::new().prefix("moye-plugin-").tempdir()?;
        let output_dir = scratch.path().join(OUTPUT_FOLDER);
        fs::create_dir(&output_dir)?;
        let request = Request {
            protocol: PROTOCOL_VERSION,
            data_dir: plugin.data_dir.canonicalize()?,
            output_dir: output_dir.canonicalize()?,
            call,
        };
        // Files let a slow or stalled plugin be cancelled without blocking on either pipe.
        let mut stdin = tempfile::tempfile()?;
        serde_json::to_writer(&mut stdin, &request)?;
        stdin.write_all(b"\n")?;
        stdin.rewind()?;
        let events_path = scratch.path().join("events.jsonl");
        let stdout = File::create(&events_path)?;
        let events = File::open(events_path)?;
        let mut command = Command::new(&plugin.executable);
        command
            .current_dir(plugin.executable.parent().context("无法定位插件目录")?)
            .env_remove("MOYE_DATA_DIR")
            .stdin(Stdio::from(stdin))
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::null());
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        let child = command
            .spawn()
            .with_context(|| format!("无法启动「{}」", plugin.name))?;
        Ok(Self {
            child,
            events,
            buffer: Vec::new(),
            read: 0,
            result: None,
            progress: None,
            scratch: Some(scratch),
            started: Instant::now(),
            timeout,
        })
    }

    pub fn poll(&mut self) -> Result<Option<Value>> {
        ensure!(self.started.elapsed() < self.timeout, "插件请求超时");
        let status = self.child.try_wait()?;
        self.read += (&mut self.events)
            .take(MAX_EVENTS - self.read + 1)
            .read_to_end(&mut self.buffer)? as u64;
        ensure!(self.read <= MAX_EVENTS, "插件输出超过大小限制");
        let mut consumed = 0;
        for line in self.buffer.split_inclusive(|byte| *byte == b'\n') {
            if !line.ends_with(b"\n") {
                break;
            }
            consumed += line.len();
            if line.iter().all(u8::is_ascii_whitespace) {
                continue;
            }
            ensure!(self.result.is_none(), "插件结束后仍有输出");
            match serde_json::from_slice(line).context("插件返回格式无效")? {
                Event::Progress {
                    message,
                    completed,
                    total,
                } => {
                    let message = message.chars().take(240).collect::<String>();
                    self.progress = Some(match (completed, total) {
                        (Some(done), Some(total)) if total > 0 => {
                            let percent = done.min(total).saturating_mul(100) / total;
                            format!("{message} · {percent}%")
                        }
                        _ => message,
                    });
                }
                Event::Result { value } => self.result = Some(value),
                Event::Error { message } => {
                    bail!("{}", message.chars().take(500).collect::<String>())
                }
            }
        }
        self.buffer.drain(..consumed);
        let Some(status) = status else {
            return Ok(None);
        };
        ensure!(status.success(), "插件已退出（{status}）");
        ensure!(self.buffer.is_empty(), "插件输出未结束");
        Ok(Some(self.result.take().context("插件未返回结果")?))
    }

    pub fn progress(&mut self) -> Option<String> {
        self.progress.take()
    }

    pub fn downloads(&mut self, value: Value, expected: usize) -> Result<(Vec<PathBuf>, TempDir)> {
        let files: Download = serde_json::from_value(value).context("插件下载结果无效")?;
        let scratch = self.scratch.take().context("插件下载目录不可用")?;
        let paths = download_paths(&scratch.path().join(OUTPUT_FOLDER), files, expected)?;
        Ok((paths, scratch))
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        if let Ok(None) = self.child.try_wait() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

pub(crate) fn listing(value: Value) -> Result<Listing> {
    let listing: Listing = serde_json::from_value(value).context("插件书籍列表无效")?;
    let mut ids = HashSet::new();
    ensure!(
        listing.items.len() <= MAX_PAGE_SIZE
            && listing.items.iter().all(|item| {
                !item.id.is_empty()
                    && item.id.len() <= 8192
                    && ids.insert(&item.id)
                    && !item.name.trim().is_empty()
                    && item.name.chars().count() <= 255
                    && !item.name.chars().any(char::is_control)
            })
            && listing
                .next_cursor
                .as_ref()
                .is_none_or(|cursor| !cursor.is_empty() && cursor.len() <= 8192),
        "插件书籍列表超出协议限制"
    );
    Ok(listing)
}

fn download_paths(dir: &Path, result: Download, expected: usize) -> Result<Vec<PathBuf>> {
    ensure!(
        expected > 0 && result.files.len() == expected,
        "插件返回的书籍数量不符"
    );
    let canonical_dir = dir.canonicalize()?;
    ensure!(fs::symlink_metadata(dir)?.is_dir(), "插件下载目录无效");
    let mut seen = HashSet::new();
    result
        .files
        .into_iter()
        .map(|name| {
            ensure!(
                !name.is_empty()
                    && !name.contains(['/', '\\', ':', '\0'])
                    && name != "."
                    && name != "..",
                "插件返回了无效的文件名"
            );
            let path = dir.join(name);
            let metadata = fs::symlink_metadata(&path).context("找不到插件下载的书籍")?;
            ensure!(
                metadata.is_file()
                    && path.canonicalize()?.parent() == Some(canonical_dir.as_path()),
                "插件下载结果必须是下载目录中的普通文件"
            );
            ensure!(
                metadata.len() <= moye_plugin_api::MAX_BOOK_BYTES,
                "插件下载的书籍超过导入上限"
            );
            ensure!(seen.insert(path.clone()), "插件返回了重复的书籍");
            Ok(path)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package(root: &Path, id: &str, version: u32) -> PathBuf {
        let folder = root.join("plugins").join(id);
        fs::create_dir_all(&folder).unwrap();
        let manifest = Manifest {
            protocol: version,
            id: id.to_owned(),
            name: id.to_owned(),
            executable: "source".to_owned(),
            capabilities: vec!["source.list".to_owned(), "source.fetch".to_owned()],
        };
        serde_json::to_writer(File::create(folder.join("plugin.json")).unwrap(), &manifest)
            .unwrap();
        let executable = folder.join(if cfg!(windows) {
            "source.exe"
        } else {
            "source"
        });
        File::create(&executable).unwrap();
        executable
    }

    #[test]
    fn discovery_ignores_missing_directory_and_reports_incompatible_plugins() {
        let root = tempfile::tempdir().unwrap();
        assert!(discover(root.path()).unwrap().plugins.is_empty());
        package(root.path(), "compatible", PROTOCOL_VERSION);
        package(root.path(), "newer", PROTOCOL_VERSION + 1);
        let catalog = discover(root.path()).unwrap();
        assert_eq!(catalog.plugins.len(), 1);
        assert_eq!(catalog.plugins[0].id, "compatible");
        assert_eq!(catalog.errors.len(), 1);
        assert!(catalog.errors[0].contains("newer"));
        assert!(!root.path().join("plugin-data").exists());
    }

    #[test]
    fn download_only_imports_files_from_the_staging_directory() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("示例.txt"), "正文").unwrap();
        let download = |name: &str| Download {
            files: vec![name.to_owned()],
        };
        assert_eq!(
            download_paths(root.path(), download("示例.txt"), 1).unwrap(),
            [root.path().join("示例.txt")]
        );
        for name in [
            "../示例.txt",
            "x/示例.txt",
            "x\\示例.txt",
            "C:示例.txt",
            "missing.txt",
        ] {
            assert!(download_paths(root.path(), download(name), 1).is_err());
        }
        assert!(download_paths(root.path(), download("示例.txt"), 2).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn dropping_an_active_request_cleans_up_its_staging_directory() {
        use std::os::unix::fs::PermissionsExt;

        let root = tempfile::tempdir().unwrap();
        let executable = package(root.path(), "slow", PROTOCOL_VERSION);
        fs::write(&executable, "#!/bin/sh\nexec /bin/sleep 60\n").unwrap();
        fs::set_permissions(executable, fs::Permissions::from_mode(0o700)).unwrap();
        let plugin = discover(root.path()).unwrap().plugins.pop().unwrap();
        let mut job = Running::start(
            plugin,
            Call::Fetch {
                ids: vec!["book".to_owned()],
            },
        )
        .unwrap();
        let scratch = job.scratch.as_ref().unwrap().path().to_owned();
        assert!(job.poll().unwrap().is_none());
        drop(job);
        assert!(!scratch.exists());
    }
}
