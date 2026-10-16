use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .expect("canonicalize repo root")
}

pub fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_agent-brain"))
}

#[derive(Debug, Clone)]
pub struct RunOut {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

pub struct Sandbox {
    pub dir: tempfile::TempDir,
    pub root: PathBuf,
    pub home: PathBuf,
    pub xdg_config: PathBuf,
    pub xdg_data: PathBuf,
    pub db_path: PathBuf,
    pub config_path: PathBuf,
    pub runs: PathBuf,
    pub personal_dir: PathBuf,
    pub repos: PathBuf,
    pub record_file: PathBuf,
    pub scenario_file: PathBuf,
    pub git_config: PathBuf,
}

impl Sandbox {
    pub fn new() -> Sandbox {
        let dir = tempfile::tempdir().expect("create tempdir");
        let root = dir.path().canonicalize().expect("canonicalize root");
        let home = root.join("home");
        let xdg_config = root.join("xdg_config");
        let xdg_data = root.join("xdg_data");
        let runs = root.join("runs");
        let personal_dir = root.join("personal_cfg");
        let repos = root.join("repos");

        for d in [&home, &xdg_config, &xdg_data, &runs, &personal_dir, &repos] {
            fs::create_dir_all(d).expect("create dir");
        }

        let db_path = xdg_data.join("agent-brain").join("brain.db");
        let config_path = root.join("brain_config.json");
        let record_file = root.join("fake_claude_record.jsonl");
        let scenario_file = root.join("fake_claude_scenario.json");
        let git_config = root.join("temp_gitconfig");

        fs::write(
            &git_config,
            "[user]\n  name = Sandbox User\n  email = sandbox@example.com\n[commit]\n  gpgsign = false\n[init]\n  defaultBranch = main\n",
        )
        .expect("write gitconfig");

        let fake = Self::fake_claude();
        if let Ok(metadata) = fs::metadata(&fake) {
            let mut perms = metadata.permissions();
            perms.set_mode(0o755);
            let _ = fs::set_permissions(&fake, perms);
        }

        let sb = Sandbox {
            dir,
            root,
            home,
            xdg_config,
            xdg_data,
            db_path,
            config_path,
            runs,
            personal_dir,
            repos,
            record_file,
            scenario_file,
            git_config,
        };

        sb.write_config(serde_json::json!({}));
        sb
    }

    pub fn write_config(&self, overrides: serde_json::Value) {
        let mut cfg = serde_json::json!({
            "claude_bin": Self::fake_claude().to_str().unwrap(),
            "models": {
                "lead": "opus",
                "dev": "sonnet",
                "qa": "sonnet"
            },
            "inspect_model": "sonnet",
            "profiles": {
                "work": {
                    "claude_config_dir": null,
                    "unset_env": [
                        "ANTHROPIC_API_KEY",
                        "ANTHROPIC_AUTH_TOKEN",
                        "ANTHROPIC_BASE_URL"
                    ]
                },
                "personal": {
                    "claude_config_dir": self.personal_dir.to_str().unwrap(),
                    "unset_env": [
                        "ANTHROPIC_API_KEY",
                        "ANTHROPIC_AUTH_TOKEN",
                        "ANTHROPIC_BASE_URL"
                    ]
                }
            },
            "common_args": ["--strict-mcp-config"],
            "max_turns": {
                "lead": null,
                "dev": null,
                "qa": null
            },
            "timeout_seconds": {
                "lead": 30,
                "dev": 30,
                "qa": 30,
                "inspect": 30
            },
            "runs_dir": self.runs.to_str().unwrap(),
            "kill_grace_secs": 1,
            "inspect_max_diff_bytes": 100000
        });

        if let Some(obj) = overrides.as_object() {
            let cfg_obj = cfg.as_object_mut().unwrap();
            for (k, v) in obj {
                cfg_obj.insert(k.clone(), v.clone());
            }
        }

        fs::write(
            &self.config_path,
            serde_json::to_string_pretty(&cfg).unwrap(),
        )
        .expect("write config");
    }

    pub fn fake_claude() -> PathBuf {
        repo_root()
            .join("tests")
            .join("fakes")
            .join("fake_claude.py")
            .canonicalize()
            .expect("canonicalize fake_claude.py")
    }

    pub fn make_repo(
        &self,
        name: &str,
        files: &[(&str, &str)],
        root: Option<&Path>,
        remote: Option<&str>,
    ) -> PathBuf {
        let target = root.unwrap_or(&self.repos).join(name);
        fs::create_dir_all(&target).expect("create repo dir");

        let mut init_cmd = std::process::Command::new("git");
        init_cmd.args(["init", "-b", "main"]);
        init_cmd.current_dir(&target);
        init_cmd.env_clear();
        if let Ok(p) = std::env::var("PATH") {
            init_cmd.env("PATH", p);
        }
        init_cmd.env("HOME", &self.home);
        init_cmd.env("GIT_CONFIG_GLOBAL", &self.git_config);
        init_cmd.env("GIT_CONFIG_NOSYSTEM", "1");
        let init_res = init_cmd.output().expect("git init");
        assert!(init_res.status.success(), "git init failed");

        if files.is_empty() {
            fs::write(target.join("README.md"), "# demo\n").expect("write README.md");
        } else {
            for (rel, content) in files {
                let fpath = target.join(rel);
                if let Some(p) = fpath.parent() {
                    fs::create_dir_all(p).expect("create parent dir");
                }
                fs::write(fpath, content).expect("write file");
            }
        }

        self.git(&target, &["add", "-A"]);
        self.git(&target, &["commit", "-m", "init"]);

        if let Some(r) = remote {
            self.git(&target, &["remote", "add", "origin", r]);
        }

        target
    }

    pub fn project_entry(&self, repo: &Path, id: &str, profile: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "path": repo.to_str().unwrap(),
            "name_vi": id,
            "aliases": [],
            "type": "code",
            "repo": serde_json::Value::Null,
            "profile": profile,
            "missing": false
        })
    }

    pub fn write_map(&self, projects: serde_json::Value, scan_roots: &[&str]) {
        let map_path = self.map_path();
        if let Some(p) = map_path.parent() {
            fs::create_dir_all(p).expect("create map dir");
        }
        let data = serde_json::json!({
            "scan_roots": scan_roots,
            "projects": projects
        });
        fs::write(map_path, serde_json::to_string_pretty(&data).unwrap()).expect("write map");
    }

    pub fn map_path(&self) -> PathBuf {
        self.xdg_config.join("agent-brain").join("projects_map.json")
    }

    pub fn read_map(&self) -> serde_json::Value {
        let content = fs::read_to_string(self.map_path()).expect("read map");
        serde_json::from_str(&content).expect("parse map")
    }

    pub fn command(&self, args: &[&str], extra_env: &[(&str, &str)]) -> std::process::Command {
        let mut cmd = std::process::Command::new(bin());
        cmd.args(args);
        cmd.env_clear();
        if let Ok(p) = std::env::var("PATH") {
            cmd.env("PATH", p);
        }
        cmd.env("HOME", &self.home);
        cmd.env("XDG_CONFIG_HOME", &self.xdg_config);
        cmd.env("XDG_DATA_HOME", &self.xdg_data);
        cmd.env("AGENT_BRAIN_DB", &self.db_path);
        cmd.env("AGENT_BRAIN_CONFIG", &self.config_path);
        cmd.env("GIT_CONFIG_GLOBAL", &self.git_config);
        cmd.env("GIT_CONFIG_NOSYSTEM", "1");
        cmd.env("FAKE_CLAUDE_RECORD", &self.record_file);
        if self.scenario_file.is_file() {
            cmd.env("FAKE_CLAUDE_SCENARIO", &self.scenario_file);
        }
        cmd.env("CLAUDECODE", "1");
        cmd.env("CLAUDE_CODE_ENTRYPOINT", "cli");
        for (k, v) in extra_env {
            cmd.env(k, v);
        }
        cmd
    }

    pub fn run(&self, args: &[&str], scenario: Option<serde_json::Value>) -> RunOut {
        self.run_env(args, scenario, &[])
    }

    pub fn run_env(
        &self,
        args: &[&str],
        scenario: Option<serde_json::Value>,
        extra_env: &[(&str, &str)],
    ) -> RunOut {
        if let Some(sc) = scenario {
            fs::write(&self.scenario_file, serde_json::to_string(&sc).unwrap())
                .expect("write scenario");
        } else {
            let _ = fs::remove_file(&self.scenario_file);
        }

        let mut cmd = self.command(args, extra_env);
        let output = cmd.output().expect("execute agent-brain");
        RunOut {
            code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        }
    }

    pub fn spawn_env(
        &self,
        args: &[&str],
        scenario: Option<serde_json::Value>,
        extra_env: &[(&str, &str)],
    ) -> std::process::Child {
        if let Some(sc) = scenario {
            fs::write(&self.scenario_file, serde_json::to_string(&sc).unwrap())
                .expect("write scenario");
        } else {
            let _ = fs::remove_file(&self.scenario_file);
        }

        self.command(args, extra_env)
            .spawn()
            .expect("spawn agent-brain")
    }

    pub fn records(&self) -> Vec<serde_json::Value> {
        if !self.record_file.is_file() {
            return Vec::new();
        }
        let content = fs::read_to_string(&self.record_file).expect("read record file");
        content
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .map(|l| serde_json::from_str(l).expect("parse record json"))
            .collect()
    }

    pub fn git(&self, repo: &Path, args: &[&str]) -> String {
        let mut cmd = std::process::Command::new("git");
        cmd.args(args);
        cmd.current_dir(repo);
        cmd.env_clear();
        if let Ok(p) = std::env::var("PATH") {
            cmd.env("PATH", p);
        }
        cmd.env("HOME", &self.home);
        cmd.env("GIT_CONFIG_GLOBAL", &self.git_config);
        cmd.env("GIT_CONFIG_NOSYSTEM", "1");
        let out = cmd.output().expect("git output");
        if !out.status.success() {
            panic!(
                "git {:?} in {} failed: {}",
                args,
                repo.display(),
                String::from_utf8_lossy(&out.stderr)
            );
        }
        String::from_utf8_lossy(&out.stdout).to_string()
    }

    pub fn db(&self) -> rusqlite::Connection {
        rusqlite::Connection::open(&self.db_path).expect("open sqlite connection")
    }
}
