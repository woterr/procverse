use std::collections::HashMap;
use std::fs;
use std::sync::mpsc::Sender;
use std::thread;
use std::time::Duration;

pub struct ProcessInfo {
    pub ppid: u32,
    pub name: String,
    pub exe: String,
    pub threads: u32,
    pub memory: u64,
    pub start_time: u64,
    pub cpu_usage: f32,
}

pub struct ProcessSnapshot {
    pub processes: HashMap<u32, ProcessInfo>,
}

pub fn run_scanner(tx: Sender<ProcessSnapshot>) {
    loop {
        let mut snapshot = HashMap::new();
        if let Ok(entries) = fs::read_dir("/proc") {
            for entry in entries.flatten() {
                if let Ok(file_name) = entry.file_name().into_string() {
                    if let Ok(pid) = file_name.parse::<u32>() {
                        if let Some(info) = parse_stat(pid) {
                            snapshot.insert(pid, info);
                        }
                    }
                }
            }
        }
        if tx
            .send(ProcessSnapshot {
                processes: snapshot,
            })
            .is_err()
        {
            break;
        }
        thread::sleep(Duration::from_millis(500));
    }
}

fn parse_stat(pid: u32) -> Option<ProcessInfo> {
    let stat_path = format!("/proc/{}/stat", pid);
    let stat_content = fs::read_to_string(&stat_path).ok()?;

    let start_paren = stat_content.find('(')?;
    let end_paren = stat_content.rfind(')')?;

    let name = stat_content[start_paren + 1..end_paren].to_string();

    let after_paren = &stat_content[end_paren + 2..];
    let mut parts = after_paren.split_whitespace();

    parts.next()?;
    let ppid: u32 = parts.next()?.parse().ok()?;

    for _ in 0..15 {
        parts.next()?;
    }

    let threads: u32 = parts.next()?.parse().unwrap_or(1);
    parts.next()?;
    let start_time: u64 = parts.next()?.parse().unwrap_or(0);

    let memory = if let Ok(statm) = fs::read_to_string(format!("/proc/{}/statm", pid)) {
        statm
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0)
            * 4096
    } else {
        0
    };

    let exe_path = format!("/proc/{}/exe", pid);
    let exe = fs::read_link(&exe_path)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| String::new());

    Some(ProcessInfo {
        ppid,
        name,
        exe,
        threads,
        memory,
        start_time,
        cpu_usage: 0.0,
    })
}
