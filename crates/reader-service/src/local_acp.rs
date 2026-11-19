//! Agent commands and npm installations owned by the local Reader profile.
//! API handlers must authorize the local desktop before exposing these operations.
mod permission;
mod protocol;
#[cfg(test)]
mod tests;

pub use permission::permission_response;
pub use protocol::{run, run_interactive};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::Mutex,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentConfig {
    pub id: String,
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
}

struct Distribution {
    id: &'static str,
    name: &'static str,
    binary: &'static str,
    args: &'static [&'static str],
    package: &'static str,
    login: &'static str,
}

const BUILTINS: &[Distribution] = &[
    Distribution {
        id: "opencode",
        name: "OpenCode",
        binary: "opencode",
        args: &["acp"],
        package: "opencode-ai",
        login: "opencode auth login",
    },
    Distribution {
        id: "codex-acp",
        name: "Codex",
        binary: "codex-acp",
        args: &[],
        package: "@agentclientprotocol/codex-acp",
        login: "codex login",
    },
];

#[derive(Default, Deserialize, Serialize)]
struct Saved {
    agents: BTreeMap<String, AgentConfig>,
    disconnected: BTreeSet<String>,
    #[serde(default)]
    selected_agent_id: Option<String>,
}

impl Saved {
    fn load(root: &Path) -> Result<Self, String> {
        match fs::read(root.join("local-agents.json")) {
            Ok(bytes) => {
                serde_json::from_slice(&bytes).map_err(|e| format!("读取本机 Agent 配置失败：{e}"))
            }
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.to_string()),
        }
    }

    fn write(&self, root: &Path) -> Result<(), String> {
        fs::create_dir_all(root).map_err(|e| e.to_string())?;
        let mut temp = tempfile::NamedTempFile::new_in(root).map_err(|e| e.to_string())?;
        serde_json::to_writer_pretty(&mut temp, self).map_err(|e| e.to_string())?;
        temp.flush().map_err(|e| e.to_string())?;
        temp.persist(root.join("local-agents.json"))
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn config(&self, root: &Path, id: &str) -> Result<AgentConfig, String> {
        if let Some(config) = self.agents.get(id) {
            return Ok(config.clone());
        }
        let d = distribution(id)?;
        let managed = managed_binary(root, d);
        Ok(AgentConfig {
            id: id.into(),
            name: d.name.into(),
            command: managed.map_or_else(
                || d.binary.into(),
                |path| path.to_string_lossy().into_owned(),
            ),
            args: d.args.iter().map(|s| s.to_string()).collect(),
        })
    }
}

fn distribution(id: &str) -> Result<&'static Distribution, String> {
    BUILTINS
        .iter()
        .find(|d| d.id == id)
        .ok_or_else(|| format!("没有受管安装项：{id}"))
}

fn executable(path: &Path) -> Option<PathBuf> {
    #[cfg(windows)]
    {
        if path.extension().is_none() {
            for ext in ["exe", "cmd", "bat", "com"] {
                let candidate = path.with_extension(ext);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
            return None;
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if fs::metadata(path).ok()?.permissions().mode() & 0o111 == 0 {
            return None;
        }
    }
    path.is_file().then(|| path.to_owned())
}

fn find_command(command: &str) -> Option<PathBuf> {
    let path = Path::new(command);
    if path.is_absolute() {
        return executable(path);
    }
    if path.components().count() != 1 {
        return None;
    }
    agent_paths()
        .into_iter()
        .find_map(|dir| executable(&dir.join(path)))
}

// A desktop launched by Finder need not inherit the user's terminal PATH.
fn agent_paths() -> Vec<PathBuf> {
    let mut paths: Vec<_> = env::split_paths(&env::var_os("PATH").unwrap_or_default())
        .filter(|dir| dir.is_absolute())
        .collect();
    #[cfg(windows)]
    if let Some(appdata) = env::var_os("APPDATA") {
        paths.push(PathBuf::from(appdata).join("npm"));
    }
    #[cfg(unix)]
    if let Some(home) = env::var_os("HOME") {
        let home = PathBuf::from(home);
        paths.extend([home.join(".local/bin"), home.join(".opencode/bin")]);
    }
    #[cfg(target_os = "macos")]
    paths.extend([
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
    ]);
    paths
}

fn package_prefix(root: &Path, d: &Distribution) -> PathBuf {
    root.join("agent-packages").join(d.id)
}

fn managed_binary(root: &Path, d: &Distribution) -> Option<PathBuf> {
    executable(
        &package_prefix(root, d)
            .join("node_modules/.bin")
            .join(d.binary),
    )
}

fn hidden(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    #[cfg(not(windows))]
    let _ = command;
}

fn agent_environment(command: &mut Command) {
    // npm shims and adapters start Node and child MCP servers by executable name.
    if let Ok(path) = env::join_paths(agent_paths()) {
        command.env("PATH", path);
    }
    hidden(command);
}

fn npm(root: &Path, d: &Distribution, install: bool) -> Result<(), String> {
    let bin = find_command("npm").ok_or("管理安装需要 npm。也可以连接已有 Agent 的可执行文件。")?;
    let prefix = std::path::absolute(package_prefix(root, d)).map_err(|e| e.to_string())?;
    fs::create_dir_all(&prefix).map_err(|e| e.to_string())?;
    let mut command = Command::new(bin);
    agent_environment(&mut command);
    // npm mutates only this agent's package tree. Switching external installations
    // and disconnecting a profile never run npm uninstall on the user's PATH.
    let package = if install {
        format!("{}@latest", d.package)
    } else {
        d.package.into()
    };
    let output = command
        .current_dir(&prefix)
        .arg(if install { "install" } else { "uninstall" })
        .arg("--prefix")
        .arg(&prefix)
        .args(["--no-audit", "--no-fund", "--loglevel=error"])
        .arg(package)
        .output()
        .map_err(|e| format!("启动 npm 失败：{e}"))?;
    if !output.status.success() {
        return Err(format!(
            "npm 操作失败（{}）：{}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    if managed_binary(root, d).is_some() != install {
        return Err(format!(
            "请核对 {} 的安装结果：{}",
            d.name,
            prefix.display()
        ));
    }
    Ok(())
}

pub fn settings(root: &Path) -> Result<Value, String> {
    let saved = Saved::load(root)?;
    let ids: BTreeSet<_> = BUILTINS
        .iter()
        .map(|d| d.id)
        .chain(saved.agents.keys().map(String::as_str))
        .collect();
    let mut agents = Vec::new();
    for id in ids {
        let config = saved.config(root, id)?;
        let d = distribution(id).ok();
        let available = find_command(&config.command).is_some();
        let mut row = serde_json::to_value(&config).map_err(|e| e.to_string())?;
        row["available"] = json!(available);
        row["configured"] =
            json!(!saved.disconnected.contains(id) && (available || saved.agents.contains_key(id)));
        row["managed"] = json!(d.is_some_and(|d| managed_binary(root, d).is_some()));
        row["installable"] = json!(d.is_some());
        row["login"] = json!(d.map(|d| login_command(root, d)));
        agents.push(row);
    }
    Ok(json!({"agents":agents,"selected_agent_id":saved.selected_agent_id}))
}

fn login_command(root: &Path, d: &Distribution) -> String {
    let (binary, args) = d.login.split_once(' ').expect("login command");
    let Some(path) = executable(
        &package_prefix(root, d)
            .join("node_modules/.bin")
            .join(binary),
    ) else {
        return d.login.to_owned();
    };
    #[cfg(windows)]
    return format!(
        "& '{}' {args}",
        path.display().to_string().replace('\'', "''")
    );
    #[cfg(not(windows))]
    return format!(
        "'{}' {args}",
        path.display().to_string().replace('\'', "'\\''")
    );
}

pub fn resolve(root: &Path, id: &str) -> Result<AgentConfig, String> {
    let saved = Saved::load(root)?;
    if saved.disconnected.contains(id) {
        return Err("请先重新连接该 Agent。".into());
    }
    let config = saved.config(root, id)?;
    if find_command(&config.command).is_none() {
        return Err(format!(
            "找不到 {} 的启动命令：{}",
            config.name, config.command
        ));
    }
    Ok(config)
}

pub fn update(root: &Path, input: &Value) -> Result<Value, String> {
    static MUTATE: Mutex<()> = Mutex::new(());
    let action = input["action"].as_str().ok_or("缺少 Agent 操作")?;
    let id = input["id"]
        .as_str()
        .or_else(|| input.pointer("/agent/id").and_then(Value::as_str))
        .unwrap_or("");
    if id.trim().is_empty() && action != "select" {
        return Err("缺少 Agent 名称标识".into());
    }
    if action == "probe" {
        let config = Saved::load(root)?.config(root, id)?;
        let probe = protocol::probe(&config)?;
        let mut state = settings(root)?;
        state["probe"] = json!({"id":id, "agent_info":probe["agentInfo"], "capabilities":probe["agentCapabilities"], "auth_methods":probe["authMethods"]});
        return Ok(state);
    }
    let _lock = MUTATE.lock().map_err(|_| "Agent 设置正忙")?;
    let mut saved = Saved::load(root)?;
    match action {
        "select" => {
            saved.selected_agent_id = if input.get("id") == Some(&Value::Null) {
                None
            } else {
                resolve(root, id)?;
                Some(id.into())
            };
        }
        "save" => {
            let config: AgentConfig =
                serde_json::from_value(input["agent"].clone()).map_err(|e| e.to_string())?;
            if config.id != id || config.name.trim().is_empty() || config.command.trim().is_empty()
            {
                return Err("请填写 Agent 名称和启动命令。".into());
            }
            let command = Path::new(&config.command);
            if !command.is_absolute() && command.components().count() != 1 {
                return Err("请填写可执行文件的完整路径或 PATH 中的命令。".into());
            }
            saved.agents.insert(id.to_owned(), config);
            saved.disconnected.remove(id);
        }
        "connect" => {
            let config = saved.config(root, id)?;
            if find_command(&config.command).is_none() {
                return Err(format!("找不到启动命令：{}", config.command));
            }
            saved.disconnected.remove(id);
        }
        "disconnect" => {
            saved.config(root, id)?;
            saved.disconnected.insert(id.to_owned());
        }
        "remove" => {
            if saved.agents.remove(id).is_none() {
                return Err("没有自定义配置可移除".into());
            }
            saved.disconnected.remove(id);
        }
        "install" | "update" | "uninstall" => {
            let d = distribution(id)?;
            if action != "install" && managed_binary(root, d).is_none() {
                return Err("该 Agent 由应用外部安装。".into());
            }
            npm(root, d, action != "uninstall")?;
            saved.agents.remove(id);
            if action == "uninstall" {
                saved.disconnected.insert(id.into());
            } else {
                saved.disconnected.remove(id);
            }
        }
        _ => return Err(format!("未知 Agent 设置操作：{action}")),
    }
    if matches!(action, "disconnect" | "uninstall" | "remove")
        && saved.selected_agent_id.as_deref() == Some(id)
    {
        saved.selected_agent_id = None;
    }
    saved.write(root)?;
    settings(root)
}
