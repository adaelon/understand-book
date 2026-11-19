use base64::Engine;
use serde::Deserialize;
use serde_json::{json, Value};
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

#[derive(Deserialize)]
struct Metadata {
    name: String,
    description: String,
}

type Result<T> = std::result::Result<T, String>;

fn safe_name(s: &str) -> bool {
    super::is_skill_id(s)
}

fn read_skill(dir: &Path) -> Result<(Metadata, String)> {
    let raw = fs::read_to_string(dir.join("SKILL.md")).map_err(|e| format!("SKILL.md: {e}"))?;
    let raw = raw.trim_start_matches('\u{feff}').replace("\r\n", "\n");
    let mut lines = raw.lines();
    if lines.next() != Some("---") {
        return Err("SKILL.md must begin with YAML frontmatter".into());
    }
    let mut header = Vec::new();
    loop {
        match lines.next() {
            Some("---") => break,
            Some(line) => header.push(line),
            None => return Err("SKILL.md frontmatter must end with ---".into()),
        }
    }
    let meta: Metadata = serde_yaml_ng::from_str(&header.join("\n")).map_err(|e| e.to_string())?;
    if !safe_name(&meta.name)
        || meta.description.trim().is_empty()
        || meta.description.chars().count() > 1024
    {
        return Err("Invalid Skill name or description".into());
    }
    let body = lines.collect::<Vec<_>>().join("\n");
    if body.trim().is_empty() {
        return Err("SKILL.md has no instructions".into());
    }
    Ok((meta, body))
}

pub(super) fn workspace(root: &Path, scope: &str) -> PathBuf {
    // Some ACP adapters interpret cwd as a URL. Percent escapes in Windows paths
    // would turn a saved scope separator back into an invalid filename.
    let mut key = String::from("scope-");
    for byte in scope.bytes() {
        if byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' {
            key.push(char::from(byte));
        } else {
            key.push_str(&format!("~{byte:02x}"));
        }
    }
    root.join("learning-skills").join(key)
}

fn location(workspace: &Path, enabled: bool) -> PathBuf {
    workspace.join(if enabled {
        ".agents/skills"
    } else {
        "available"
    })
}

fn entries(workspace: &Path, enabled: bool) -> Result<Vec<PathBuf>> {
    let dir = location(workspace, enabled);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut paths = Vec::new();
    for item in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let path = item.map_err(|e| e.to_string())?.path();
        if path.is_dir() {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

pub(super) fn context(workspace: &Path) -> Result<String> {
    let mut instructions = String::new();
    for path in entries(workspace, true)? {
        let (meta, body) = read_skill(&path)?;
        instructions.push_str(&format!(
            "\n\nTeaching skill selected by the reader: {}\n{}\nResources: {}\n{}",
            meta.name,
            meta.description,
            path.display(),
            body
        ));
    }
    Ok(instructions)
}

pub(super) fn list(workspace: &Path) -> Result<Value> {
    let mut skills = Vec::new();
    for enabled in [true, false] {
        for path in entries(workspace, enabled)? {
            let (meta, _) = read_skill(&path)?;
            skills.push(json!({"name":meta.name,"description":meta.description,"enabled":enabled,"mode":"instructions"}));
        }
    }
    Ok(json!({"skills":skills}))
}

fn copy_tree(src: &Path, dst: &Path) -> Result<()> {
    for child in fs::read_dir(src).map_err(|e| e.to_string())? {
        let child = child.map_err(|e| e.to_string())?;
        if child.file_name() == ".git" {
            continue;
        }
        let dest = dst.join(child.file_name());
        let kind = child.file_type().map_err(|e| e.to_string())?;
        if kind.is_dir() {
            fs::create_dir(&dest).map_err(|e| e.to_string())?;
            copy_tree(&child.path(), &dest)?;
        } else if kind.is_file() {
            fs::copy(child.path(), dest).map_err(|e| e.to_string())?;
        } else {
            return Err(format!(
                "Unsupported Skill resource {}",
                child.path().display()
            ));
        }
    }
    Ok(())
}

fn contents(
    client: &ureq::Agent,
    repo: &str,
    path: &str,
    revision: Option<&str>,
) -> Result<Vec<Value>> {
    let mut api = url::Url::parse("https://api.github.com/repos/").unwrap();
    {
        let mut parts = api.path_segments_mut().unwrap();
        parts
            .pop_if_empty()
            .extend(repo.split('/'))
            .push("contents");
        if !path.is_empty() {
            parts.extend(path.split('/'));
        }
    }
    if let Some(rev) = revision {
        api.query_pairs_mut().append_pair("ref", rev);
    }
    client
        .get(api.as_str())
        .call()
        .map_err(|e| e.to_string())?
        .into_json()
        .map_err(|e| e.to_string())
}

fn download_file(
    client: &ureq::Agent,
    item: &Value,
    target: &Path,
    raw_available: &mut bool,
) -> Result<()> {
    if *raw_available {
        let url = item["download_url"]
            .as_str()
            .ok_or("Missing GitHub download URL")?;
        if let Ok(response) = client.get(url).call() {
            let mut file = fs::File::create(target).map_err(|e| e.to_string())?;
            if std::io::copy(&mut response.into_reader(), &mut file).is_ok() {
                return file.flush().map_err(|e| e.to_string());
            }
        }
        // The public raw-content CDN and GitHub's API can have different connectivity.
        // Subsequent resources use the API as well, avoiding a timeout per file.
        *raw_available = false;
    }
    let blob_url = item["git_url"].as_str().ok_or("Missing GitHub blob URL")?;
    let blob: Value = client
        .get(blob_url)
        .call()
        .map_err(|e| e.to_string())?
        .into_json()
        .map_err(|e| e.to_string())?;
    let encoded = blob["content"]
        .as_str()
        .filter(|_| blob["encoding"] == "base64")
        .ok_or("GitHub returned no file content")?;
    let encoded: Vec<_> = encoded
        .bytes()
        .filter(|b| !b.is_ascii_whitespace())
        .collect();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|e| e.to_string())?;
    fs::write(target, bytes).map_err(|e| e.to_string())
}

fn download_tree(
    client: &ureq::Agent,
    repo: &str,
    revision: Option<&str>,
    items: Vec<Value>,
    dest: &Path,
    raw_available: &mut bool,
) -> Result<()> {
    for item in items {
        let name = item["name"]
            .as_str()
            .ok_or("Missing GitHub resource name")?;
        if Path::new(name).components().count() != 1
            || !matches!(
                Path::new(name).components().next(),
                Some(Component::Normal(_))
            )
        {
            return Err("Invalid GitHub resource name".into());
        }
        let target = dest.join(name);
        match item["type"].as_str() {
            Some("file") => download_file(client, &item, &target, raw_available)?,
            Some("dir") => {
                fs::create_dir(&target).map_err(|e| e.to_string())?;
                let children = contents(
                    client,
                    repo,
                    item["path"].as_str().ok_or("Missing GitHub folder")?,
                    revision,
                )?;
                download_tree(client, repo, revision, children, &target, raw_available)?;
            }
            _ => return Err(format!("Unsupported GitHub resource {name}")),
        }
    }
    Ok(())
}

fn github(source: &str, subdir: &str, dest: &Path) -> Result<()> {
    let url = url::Url::parse(source).map_err(|e| e.to_string())?;
    if url.scheme() != "https" || url.host_str() != Some("github.com") {
        return Err("Use a local directory or an https://github.com repository".into());
    }
    let parts: Vec<_> = url.path().trim_matches('/').split('/').collect();
    if parts.len() < 2 || (parts.len() > 2 && (parts.len() < 4 || parts[2] != "tree")) {
        return Err("Use a GitHub repository or tree URL".into());
    }
    let repo = format!("{}/{}", parts[0], parts[1].trim_end_matches(".git"));
    let rev = parts.get(3).copied();
    let mut path = parts.get(4..).unwrap_or_default().join("/");
    if !subdir.is_empty() {
        if !path.is_empty() {
            path.push('/');
        }
        path.push_str(subdir);
    }
    let client = ureq::AgentBuilder::new()
        .user_agent("Understand-Book-Skill-Import")
        .timeout(Duration::from_secs(25))
        .build();
    let items = contents(&client, &repo, &path, rev)?;
    if !items.iter().any(|i| i["name"] == "SKILL.md") {
        return Err(format!(
            "SKILL.md missing in {repo}/{path}. Select the Skill directory."
        ));
    }
    let mut raw_available = true;
    download_tree(&client, &repo, rev, items, dest, &mut raw_available)
}

#[cfg(test)]
pub(super) fn import(workspace: &Path, source: &str, subdir: &str) -> Result<Value> {
    let (staging, name) = stage(workspace, source, subdir)?;
    install(workspace, staging, &name)
}

pub(super) fn stage(
    workspace: &Path,
    source: &str,
    subdir: &str,
) -> Result<(tempfile::TempDir, String)> {
    if Path::new(subdir)
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err("Skill subdirectory must be relative".into());
    }
    fs::create_dir_all(workspace).map_err(|e| e.to_string())?;
    let staging = tempfile::tempdir_in(workspace).map_err(|e| e.to_string())?;
    if source.starts_with("https://") {
        github(source, subdir, staging.path())?;
    } else {
        let src = Path::new(source)
            .join(subdir)
            .canonicalize()
            .map_err(|e| e.to_string())?;
        if staging
            .path()
            .canonicalize()
            .map_err(|e| e.to_string())?
            .starts_with(&src)
        {
            return Err(
                "Choose the directory containing the Skill, outside its installation folder".into(),
            );
        }
        read_skill(&src)?;
        copy_tree(&src, staging.path())?;
    }
    let (meta, _) = read_skill(staging.path())?;
    Ok((staging, meta.name))
}

pub(super) fn install(workspace: &Path, staging: tempfile::TempDir, name: &str) -> Result<Value> {
    let target = location(workspace, false).join(name);
    if target.exists() || location(workspace, true).join(name).exists() {
        return Err(format!("Skill {name} already installed"));
    }
    fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
    fs::rename(staging.path(), target).map_err(|e| e.to_string())?;
    list(workspace)
}

pub(super) fn change(workspace: &Path, name: &str, enabled: Option<bool>) -> Result<Value> {
    if !safe_name(name) {
        return Err("Invalid Skill name".into());
    }
    let active = location(workspace, true).join(name);
    let stored = location(workspace, false).join(name);
    let current = if active.is_dir() {
        &active
    } else if stored.is_dir() {
        &stored
    } else {
        return Err(format!("Skill {name} is not installed in this scope"));
    };
    if let Some(enable) = enabled {
        let target = if enable { &active } else { &stored };
        if target != current {
            fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
            fs::rename(current, target).map_err(|e| e.to_string())?;
        }
    } else {
        fs::remove_dir_all(current).map_err(|e| e.to_string())?;
    }
    list(workspace)
}
