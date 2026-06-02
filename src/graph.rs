use crate::process_scanner::ProcessSnapshot;
use glam::Vec2;
use ratatui::style::Color;
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, PartialEq)]
pub enum NodeState {
    Spawning,
    Alive,
    Dying,
    Dead,
}

#[derive(Clone, Copy)]
pub enum EventType {
    Spawn,
    Exit,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Importance {
    High,
    Medium,
    Low,
}

pub struct ProcessEvent {
    pub name: String,
    pub event_type: EventType,
}

pub struct Camera {
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
}

pub struct Node {
    pub pid: u32,
    pub ppid: u32,
    pub name: String,
    pub exe: String,
    pub threads: u32,
    pub memory: u64,
    pub start_time: u64,
    pub cpu_usage: f32,

    pub pos: Vec2,
    pub vel: Vec2,
    pub fixed: bool,
    pub color: Color,
    pub depth: u32,
    pub is_kernel_thread: bool,
    pub importance: Importance,

    pub state: NodeState,
    pub spawn_timer: f32,
    pub death_timer: f32,

    pub is_cluster: bool,
    pub cluster_family: Option<String>,
    pub cluster_expanded: bool,
    pub cluster_count: usize,
    pub is_hidden: bool,
}

pub struct Edge {
    pub from: usize,
    pub to: usize,
}

pub struct GraphDiagnostics {
    pub total_nodes: usize,
    pub total_edges: usize,
    pub total_clusters: usize,
    pub avg_dist: f32,
    pub min_x: f32,
    pub max_x: f32,
    pub min_y: f32,
    pub max_y: f32,
    pub max_depth: u32,
}

pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub camera: Camera,
    pub selected_idx: Option<usize>,
    pub focus_mode: bool,
    pub trace_mode: bool,
    pub events: VecDeque<ProcessEvent>,

    pub max_memory: u64,
    pub max_threads: u32,
    pub max_cpu: f32,
}

pub fn is_kernel_process(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.starts_with("kworker")
        || lower.starts_with("rcu")
        || lower.starts_with("migration")
        || lower.starts_with("ksoftirqd")
        || lower.starts_with("cpuhp")
        || lower.starts_with("idle_inject")
        || lower.starts_with("watchdog")
}

pub fn get_kernel_family(name: &str) -> Option<&'static str> {
    let lower = name.to_lowercase();
    if lower.starts_with("kworker") {
        Some("kworker")
    } else if lower.starts_with("rcu") {
        Some("rcu")
    } else if lower.starts_with("migration") {
        Some("migration")
    } else if lower.starts_with("ksoftirqd") {
        Some("ksoftirqd")
    } else if lower.starts_with("cpuhp") {
        Some("cpuhp")
    } else if lower.starts_with("idle_inject") {
        Some("idle_inject")
    } else if lower.starts_with("watchdog") {
        Some("watchdog")
    } else {
        None
    }
}

fn get_cluster_pid(family: &str) -> u32 {
    let mut hash = 0u32;
    for b in family.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(b as u32);
    }
    2_000_000_000 + (hash % 1_000_000)
}

fn classify_process(name: &str) -> Color {
    let lower = name.to_lowercase();
    if lower.contains("systemd") || lower == "init" {
        Color::Yellow
    } else if lower.contains("bash")
        || lower.contains("zsh")
        || lower.contains("fish")
        || lower.contains("tmux")
        || lower.contains("tty")
        || lower.contains("sh")
    {
        Color::Green
    } else if lower.contains("nvim")
        || lower.contains("vim")
        || lower.contains("nano")
        || lower.contains("code")
        || lower.contains("emacs")
    {
        Color::Cyan
    } else if lower.contains("firefox")
        || lower.contains("chrome")
        || lower.contains("brave")
        || lower.contains("edge")
        || lower.contains("webkit")
        || lower.contains("zen")
    {
        Color::Red
    } else if lower.contains("docker")
        || lower.contains("containerd")
        || lower.contains("podman")
        || lower.contains("k8s")
        || lower.contains("kube")
    {
        Color::Blue
    } else {
        Color::White
    }
}

fn classify_importance(name: &str) -> Importance {
    let lower = name.to_lowercase();
    if lower == "systemd"
        || lower == "init"
        || lower.contains("kitty")
        || lower.contains("alacritty")
        || lower.contains("wezterm")
        || lower.contains("bash")
        || lower.contains("zsh")
        || lower.contains("fish")
        || lower.contains("nvim")
        || lower.contains("vim")
        || lower.contains("code")
        || lower.contains("zed")
        || lower.contains("firefox")
        || lower.contains("chrome")
        || lower.contains("zen")
        || lower.contains("procverse")
        || lower.contains("hyprland")
        || lower.contains("gnome")
        || lower.contains("kde")
    {
        Importance::High
    } else if is_kernel_process(name)
        || lower.contains("sleep")
        || lower == "cat"
        || lower == "grep"
    {
        Importance::Low
    } else {
        Importance::Medium
    }
}

impl Graph {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            camera: Camera {
                x: 0.0,
                y: 0.0,
                zoom: 1.0,
            },
            selected_idx: Some(0),
            focus_mode: false,
            trace_mode: false,
            events: VecDeque::with_capacity(15),
            max_memory: 0,
            max_threads: 0,
            max_cpu: 0.0,
        }
    }

    pub fn compute_diagnostics(&self) -> GraphDiagnostics {
        let active_nodes = self
            .nodes
            .iter()
            .filter(|n| n.state != NodeState::Dead && !n.is_hidden)
            .collect::<Vec<_>>();
        let total_nodes = active_nodes.len();
        let total_clusters = active_nodes.iter().filter(|n| n.is_cluster).count();

        let total_edges = self
            .edges
            .iter()
            .filter(|e| {
                self.nodes[e.from].state != NodeState::Dead
                    && !self.nodes[e.from].is_hidden
                    && self.nodes[e.to].state != NodeState::Dead
                    && !self.nodes[e.to].is_hidden
            })
            .count();

        if total_nodes == 0 {
            return GraphDiagnostics {
                total_nodes: 0,
                total_edges: 0,
                total_clusters: 0,
                avg_dist: 0.0,
                min_x: 0.0,
                max_x: 0.0,
                min_y: 0.0,
                max_y: 0.0,
                max_depth: 0,
            };
        }

        let mut sum_dist = 0.0;
        let mut min_x = f32::MAX;
        let mut max_x = f32::MIN;
        let mut min_y = f32::MAX;
        let mut max_y = f32::MIN;
        let mut max_depth = 0;

        for n in &active_nodes {
            let dist = n.pos.length();
            sum_dist += dist;

            if n.pos.x < min_x {
                min_x = n.pos.x;
            }
            if n.pos.x > max_x {
                max_x = n.pos.x;
            }
            if n.pos.y < min_y {
                min_y = n.pos.y;
            }
            if n.pos.y > max_y {
                max_y = n.pos.y;
            }

            if n.depth > max_depth {
                max_depth = n.depth;
            }
        }

        GraphDiagnostics {
            total_nodes,
            total_edges,
            total_clusters,
            avg_dist: sum_dist / total_nodes as f32,
            min_x,
            max_x,
            min_y,
            max_y,
            max_depth,
        }
    }

    pub fn calculate_fit_zoom(&self) -> f32 {
        let diag = self.compute_diagnostics();
        let width = (diag.max_x - diag.min_x).max(10.0);
        let height = (diag.max_y - diag.min_y).max(10.0);

        let zoom_x = 200.0 / width;
        let zoom_y = 200.0 / height;

        zoom_x.min(zoom_y)
    }

    pub fn auto_fit_camera(&mut self) {
        let diag = self.compute_diagnostics();
        let fit_zoom = self.calculate_fit_zoom();

        self.camera.zoom = fit_zoom * 0.9;
        self.camera.x = (diag.min_x + diag.max_x) / 2.0;
        self.camera.y = (diag.min_y + diag.max_y) / 2.0;
    }

    pub fn center_on_selection(&mut self) {
        if let Some(idx) = self.selected_idx {
            if let Some(node) = self.nodes.get(idx) {
                self.camera.x = node.pos.x;
                self.camera.y = node.pos.y;
            }
        }
    }

    pub fn select_nearest(&mut self, world_x: f32, world_y: f32, userland_view: bool) {
        let click_pos = Vec2::new(world_x, world_y);
        let mut closest_idx = None;
        let mut min_dist = f32::MAX;

        for (i, node) in self.nodes.iter().enumerate() {
            if node.state == NodeState::Dead || node.is_hidden {
                continue;
            }
            if userland_view && node.is_kernel_thread {
                continue;
            }

            let dist = (node.pos - click_pos).length();
            if dist < min_dist && dist < (10.0 / self.camera.zoom.max(0.1)) {
                min_dist = dist;
                closest_idx = Some(i);
            }
        }

        if closest_idx.is_some() {
            self.selected_idx = closest_idx;
        }
    }

    pub fn log_event(&mut self, name: &str, event_type: EventType) {
        self.events.push_front(ProcessEvent {
            name: name.to_string(),
            event_type,
        });
        if self.events.len() > 15 {
            self.events.pop_back();
        }
    }

    pub fn update_from_snapshot(&mut self, snapshot: &ProcessSnapshot) {
        let is_first = self.nodes.is_empty();

        let mut exited_processes = Vec::new();
        for node in &mut self.nodes {
            if node.is_cluster || node.state == NodeState::Dead || node.state == NodeState::Dying {
                continue;
            }
            let still_alive = snapshot
                .processes
                .get(&node.pid)
                .map_or(false, |p| p.start_time == node.start_time);
            if !still_alive {
                node.state = NodeState::Dying;
                node.death_timer = 1.0;
                exited_processes.push(node.name.clone());
            } else if let Some(info) = snapshot.processes.get(&node.pid) {
                node.memory = info.memory;
                node.threads = info.threads;
                node.cpu_usage = info.cpu_usage;
            }
        }

        for name in exited_processes {
            self.log_event(&name, EventType::Exit);
        }

        for (pid, info) in &snapshot.processes {
            let exists = self.nodes.iter().any(|n| {
                n.pid == *pid && n.start_time == info.start_time && n.state != NodeState::Dead
            });
            if !exists {
                let parent_idx = self
                    .nodes
                    .iter()
                    .position(|n| n.pid == info.ppid && n.state != NodeState::Dead);

                let (spawn_pos, depth) = if let Some(pidx) = parent_idx {
                    let p = &self.nodes[pidx];
                    let angle = (*pid as f32) * 1.618;
                    let offset = Vec2::new(angle.cos(), angle.sin()) * 3.0;
                    (p.pos + offset, p.depth + 1)
                } else {
                    let angle = (*pid as f32) * 1.618;
                    let radius = if is_first {
                        10.0 + (*pid as f32 % 30.0)
                    } else {
                        50.0 + (*pid as f32 % 50.0)
                    };
                    (Vec2::new(angle.cos(), angle.sin()) * radius, 0)
                };

                let family_opt = get_kernel_family(&info.name);

                let new_node = Node {
                    pid: *pid,
                    ppid: info.ppid,
                    name: info.name.clone(),
                    exe: info.exe.clone(),
                    threads: info.threads,
                    memory: info.memory,
                    start_time: info.start_time,
                    cpu_usage: info.cpu_usage,
                    pos: spawn_pos,
                    vel: Vec2::ZERO,
                    fixed: *pid == 1,
                    color: classify_process(&info.name),
                    depth,
                    is_kernel_thread: is_kernel_process(&info.name),
                    importance: classify_importance(&info.name),
                    state: NodeState::Spawning,
                    spawn_timer: 1.0,
                    death_timer: 0.0,
                    is_cluster: false,
                    cluster_family: family_opt.map(|s| s.to_string()),
                    cluster_expanded: false,
                    cluster_count: 0,
                    is_hidden: false,
                };

                if let Some(slot) = self.nodes.iter_mut().find(|n| n.state == NodeState::Dead) {
                    *slot = new_node;
                } else {
                    self.nodes.push(new_node);
                }

                if !is_first {
                    self.log_event(&info.name, EventType::Spawn);
                }
            }
        }

        let mut family_members: HashMap<String, Vec<usize>> = HashMap::new();
        for i in 0..self.nodes.len() {
            let n = &self.nodes[i];
            if !n.is_cluster && n.state != NodeState::Dead && n.state != NodeState::Dying {
                if let Some(fam) = &n.cluster_family {
                    family_members.entry(fam.clone()).or_default().push(i);
                }
            }
        }

        let mut missing_clusters = Vec::new();
        for (fam, members) in &family_members {
            let v_pid = get_cluster_pid(fam);
            if !self
                .nodes
                .iter()
                .any(|n| n.is_cluster && n.pid == v_pid && n.state != NodeState::Dead)
            {
                let ppid = self.nodes[members[0]].ppid;
                missing_clusters.push((fam.clone(), v_pid, ppid));
            }
        }

        for (fam, v_pid, ppid) in missing_clusters {
            let parent_pos = self
                .nodes
                .iter()
                .find(|n| n.pid == ppid)
                .map(|n| n.pos)
                .unwrap_or(Vec2::ZERO);
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let new_cluster = Node {
                pid: v_pid,
                ppid,
                name: fam.clone(),
                exe: String::new(),
                threads: 0,
                memory: 0,
                start_time: now,
                cpu_usage: 0.0,
                pos: parent_pos,
                vel: Vec2::ZERO,
                fixed: false,
                color: Color::Magenta,
                depth: 0,
                is_kernel_thread: is_kernel_process(&fam),
                importance: Importance::High,
                state: NodeState::Spawning,
                spawn_timer: 1.0,
                death_timer: 0.0,
                is_cluster: true,
                cluster_family: Some(fam.clone()),
                cluster_expanded: false,
                cluster_count: 0,
                is_hidden: false,
            };
            if let Some(pos) = self.nodes.iter().position(|n| n.state == NodeState::Dead) {
                self.nodes[pos] = new_cluster;
            } else {
                self.nodes.push(new_cluster);
            }
        }

        for (fam, members) in &family_members {
            let v_pid = get_cluster_pid(fam);
            if let Some(c_idx) = self
                .nodes
                .iter()
                .position(|n| n.is_cluster && n.pid == v_pid && n.state != NodeState::Dead)
            {
                self.nodes[c_idx].cluster_count = members.len();
                let expanded = self.nodes[c_idx].cluster_expanded;
                let c_pos = self.nodes[c_idx].pos;

                let mut cluster_mem = 0;
                let mut cluster_threads = 0;

                for &m_idx in members {
                    cluster_mem += self.nodes[m_idx].memory;
                    cluster_threads += self.nodes[m_idx].threads;
                    self.nodes[m_idx].is_hidden = !expanded;
                    if !expanded {
                        self.nodes[m_idx].pos = c_pos;
                    }
                }
                self.nodes[c_idx].memory = cluster_mem;
                self.nodes[c_idx].threads = cluster_threads;
            }
        }

        for n in &mut self.nodes {
            if n.is_cluster && n.state != NodeState::Dead && n.state != NodeState::Dying {
                if let Some(fam) = &n.cluster_family {
                    if !family_members.contains_key(fam) {
                        n.state = NodeState::Dying;
                        n.death_timer = 1.0;
                    }
                }
            }
        }

        self.edges.clear();
        let init_idx = self
            .nodes
            .iter()
            .position(|n| n.pid == 1 && n.state != NodeState::Dead);

        for i in 0..self.nodes.len() {
            let n = &self.nodes[i];
            if n.state == NodeState::Dead || n.is_hidden || n.pid == 1 {
                continue;
            }

            let mut parent_found = false;

            if n.is_cluster {
                if let Some(parent_idx) = self
                    .nodes
                    .iter()
                    .position(|pn| pn.pid == n.ppid && pn.state != NodeState::Dead)
                {
                    self.edges.push(Edge {
                        from: parent_idx,
                        to: i,
                    });
                    parent_found = true;
                }
            } else if let Some(fam) = &n.cluster_family {
                let v_pid = get_cluster_pid(fam);
                if let Some(c_idx) = self
                    .nodes
                    .iter()
                    .position(|cn| cn.pid == v_pid && cn.is_cluster && cn.state != NodeState::Dead)
                {
                    self.edges.push(Edge { from: c_idx, to: i });
                    parent_found = true;
                }
            }

            if !parent_found && !n.is_cluster && n.cluster_family.is_none() {
                if let Some(parent_idx) = self
                    .nodes
                    .iter()
                    .position(|pn| pn.pid == n.ppid && pn.state != NodeState::Dead)
                {
                    self.edges.push(Edge {
                        from: parent_idx,
                        to: i,
                    });
                    parent_found = true;
                }
            }

            if !parent_found {
                if let Some(init) = init_idx {
                    if i != init {
                        self.edges.push(Edge { from: init, to: i });
                    }
                }
            }
        }

        let mut adj = vec![vec![]; self.nodes.len()];
        for e in &self.edges {
            adj[e.from].push(e.to);
        }

        let mut queue = std::collections::VecDeque::new();
        if let Some(idx) = init_idx {
            self.nodes[idx].depth = 0;
            queue.push_back(idx);
        }

        let mut visited = vec![false; self.nodes.len()];
        while let Some(curr) = queue.pop_front() {
            visited[curr] = true;
            let current_depth = self.nodes[curr].depth;
            for &child in &adj[curr] {
                if !visited[child] {
                    self.nodes[child].depth = current_depth + 1;
                    queue.push_back(child);
                }
            }
        }

        self.max_memory = 0;
        self.max_threads = 0;
        self.max_cpu = 0.0;
        for n in &self.nodes {
            if n.state != NodeState::Dead && !n.is_hidden {
                self.max_memory = self.max_memory.max(n.memory);
                self.max_threads = self.max_threads.max(n.threads);
                if n.cpu_usage > self.max_cpu {
                    self.max_cpu = n.cpu_usage;
                }
            }
        }
    }

    pub fn get_process_path(&self, start_idx: usize) -> Vec<usize> {
        let mut path = Vec::new();
        let mut current = start_idx;
        let mut visited = HashSet::new();

        loop {
            path.push(current);
            visited.insert(current);

            let mut found_parent = false;
            for edge in &self.edges {
                if edge.to == current {
                    if !visited.contains(&edge.from) {
                        current = edge.from;
                        found_parent = true;
                    }
                    break;
                }
            }
            if !found_parent {
                break;
            }
        }
        path.reverse();
        path
    }

    pub fn get_ancestors(&self, start_idx: usize) -> HashSet<usize> {
        let mut ancestors = HashSet::new();
        let mut current = start_idx;
        loop {
            let mut found = false;
            for edge in &self.edges {
                if edge.to == current {
                    ancestors.insert(edge.from);
                    current = edge.from;
                    found = true;
                    break;
                }
            }
            if !found {
                break;
            }
        }
        ancestors
    }

    pub fn get_descendants(&self, start_idx: usize) -> HashSet<usize> {
        let mut descendants = HashSet::new();
        let mut stack = vec![start_idx];

        while let Some(current) = stack.pop() {
            for edge in &self.edges {
                if edge.from == current {
                    if !descendants.contains(&edge.to) {
                        descendants.insert(edge.to);
                        stack.push(edge.to);
                    }
                }
            }
        }
        descendants
    }
}
