// Linux /proc readers: process tree, listening TCP ports, and finding a project's leftover dev servers.
use nix::{
    sys::signal::{Signal, kill},
    unistd::Pid,
};
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    fs,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

// Runtimes whose processes count as a project's dev server; matched against the executable's file name.
const RUNTIMES: &[&str] = &["node", "bun", "deno", "python", "uv", "cargo"];
// Anything running under an AI agent (its MCP servers, its shell jobs) is never touched.
const PROTECTED_ANCESTORS: &[&str] = &["claude", "codex"];
const STOP_GRACE: Duration = Duration::from_millis(1500);

pub struct Proc {
    pub ppid: i32,
    pub comm: String,
}

pub type Table = HashMap<i32, Proc>;

pub fn table() -> Table {
    let mut table = HashMap::new();
    let Ok(entries) = fs::read_dir("/proc") else {
        return table;
    };
    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|s| s.parse::<i32>().ok())
        else {
            continue;
        };
        let Ok(stat) = fs::read_to_string(format!("/proc/{pid}/stat")) else {
            continue;
        };
        // The command name sits in parentheses and may itself contain spaces or parentheses.
        let (Some(open), Some(close)) = (stat.find('('), stat.rfind(')')) else {
            continue;
        };
        let comm = stat[open + 1..close].to_owned();
        let ppid = stat[close + 1..]
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        table.insert(pid, Proc { ppid, comm });
    }
    table
}

pub fn descendants(table: &Table, root: i32) -> Vec<i32> {
    let mut children: HashMap<i32, Vec<i32>> = HashMap::new();
    for (&pid, proc) in table {
        children.entry(proc.ppid).or_default().push(pid);
    }
    let mut result = vec![root];
    let mut index = 0;
    while index < result.len() {
        if let Some(kids) = children.get(&result[index]) {
            result.extend(kids);
        }
        index += 1;
    }
    result
}

// inode -> port for every socket in LISTEN state, IPv4 and IPv6.
pub fn listeners() -> HashMap<u64, u16> {
    let mut map = HashMap::new();
    for file in ["/proc/net/tcp", "/proc/net/tcp6"] {
        let Ok(text) = fs::read_to_string(file) else {
            continue;
        };
        for line in text.lines().skip(1) {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() < 10 || fields[3] != "0A" {
                continue;
            }
            let port = fields[1]
                .rsplit(':')
                .next()
                .and_then(|p| u16::from_str_radix(p, 16).ok());
            if let (Some(port), Ok(inode)) = (port, fields[9].parse::<u64>()) {
                map.insert(inode, port);
            }
        }
    }
    map
}

fn socket_inodes(pid: i32) -> Vec<u64> {
    let Ok(entries) = fs::read_dir(format!("/proc/{pid}/fd")) else {
        return vec![];
    };
    entries
        .flatten()
        .filter_map(|e| fs::read_link(e.path()).ok())
        .filter_map(|link| {
            link.to_str()?
                .strip_prefix("socket:[")?
                .strip_suffix(']')?
                .parse()
                .ok()
        })
        .collect()
}

pub fn ports_of(pids: &[i32], listeners: &HashMap<u64, u16>) -> BTreeSet<u16> {
    pids.iter()
        .flat_map(|&pid| socket_inodes(pid))
        .filter_map(|inode| listeners.get(&inode).copied())
        .collect()
}

pub struct Stale {
    pub pid: i32,
    pub name: String,
    pub ports: BTreeSet<u16>,
    tree: Vec<i32>,
}

fn link(pid: i32, what: &str) -> Option<PathBuf> {
    let path = fs::read_link(format!("/proc/{pid}/{what}")).ok()?;
    // A replaced binary shows up as "/path (deleted)".
    let text = path.to_string_lossy();
    Some(PathBuf::from(
        text.strip_suffix(" (deleted)").unwrap_or(&text),
    ))
}

fn exe_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn under_agent(table: &Table, mut pid: i32) -> bool {
    let mut seen = HashSet::new();
    while let Some(proc) = table.get(&pid) {
        if !seen.insert(pid) || pid <= 1 {
            return false;
        }
        let exe = link(pid, "exe").map(|p| exe_name(&p)).unwrap_or_default();
        if PROTECTED_ANCESTORS
            .iter()
            .any(|name| proc.comm == *name || exe == *name)
        {
            return true;
        }
        pid = proc.ppid;
    }
    false
}

// A leftover server: my process, working inside the project, a known runtime or a project-built binary, holding a listening port, not under an AI agent.
pub fn find_stale(root: &Path, skip: &HashSet<i32>) -> Vec<Stale> {
    let Ok(me) = fs::metadata("/proc/self").map(|m| m.uid()) else {
        return vec![];
    };
    let listeners = listeners();
    if listeners.is_empty() {
        return vec![];
    }
    let table = table();
    let mut found: Vec<Stale> = vec![];
    let mut claimed: HashSet<i32> = HashSet::new();
    let mut pids: Vec<i32> = table.keys().copied().collect();
    pids.sort();
    for pid in pids {
        if skip.contains(&pid) || claimed.contains(&pid) {
            continue;
        }
        if fs::metadata(format!("/proc/{pid}")).map(|m| m.uid()).ok() != Some(me) {
            continue;
        }
        let Some(cwd) = link(pid, "cwd") else {
            continue;
        };
        if !cwd.starts_with(root) {
            continue;
        }
        let Some(exe) = link(pid, "exe") else {
            continue;
        };
        let name = exe_name(&exe);
        // Fedora names the binary node-24; report the runtime the user knows.
        let name = match RUNTIMES.iter().find(|r| name.starts_with(*r)) {
            Some(runtime) => runtime.to_string(),
            None if exe.starts_with(root) => name,
            None => continue,
        };
        let ports = ports_of(&[pid], &listeners);
        if ports.is_empty() || under_agent(&table, pid) {
            continue;
        }
        let tree = descendants(&table, pid);
        claimed.extend(&tree);
        found.push(Stale {
            pid,
            name,
            ports,
            tree,
        });
    }
    // A parent and its child can both qualify; keep only the topmost so nothing is signalled twice.
    let roots: HashSet<i32> = found.iter().map(|s| s.pid).collect();
    found.retain(|s| {
        let mut parent = table.get(&s.pid).map_or(0, |p| p.ppid);
        while parent > 1 {
            if roots.contains(&parent) {
                return false;
            }
            parent = table.get(&parent).map_or(0, |p| p.ppid);
        }
        true
    });
    found
}

fn alive(pid: i32) -> bool {
    // A zombie still answers kill(0) but has already let go of its ports.
    match fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(stat) => stat
            .rfind(')')
            .and_then(|i| stat[i + 1..].split_whitespace().next())
            .is_some_and(|state| state != "Z" && state != "X"),
        Err(_) => false,
    }
}

// Asks each leftover process tree to stop, then forces whatever is still alive after the grace period.
pub fn terminate(stale: &[Stale]) {
    let pids: Vec<i32> = stale.iter().flat_map(|s| s.tree.iter().copied()).collect();
    for &pid in &pids {
        let _ = kill(Pid::from_raw(pid), Signal::SIGTERM);
    }
    let deadline = Instant::now() + STOP_GRACE;
    while Instant::now() < deadline && pids.iter().any(|&p| alive(p)) {
        thread::sleep(Duration::from_millis(40));
    }
    for &pid in pids.iter().filter(|&&p| alive(p)) {
        let _ = kill(Pid::from_raw(pid), Signal::SIGKILL);
    }
}
