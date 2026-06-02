use crate::graph::{EventType, Graph, Importance, NodeState};
use crate::{AppMode, OverlayMode};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        canvas::{Canvas, Line as CanvasLine, Points},
        Block, Borders, Clear, Paragraph,
    },
    Frame,
};
use std::collections::HashSet;

fn adjust_color(c: Color, factor: f32) -> Color {
    if let Color::Rgb(r, g, b) = c {
        Color::Rgb(
            (r as f32 * factor) as u8,
            (g as f32 * factor) as u8,
            (b as f32 * factor) as u8,
        )
    } else {
        c
    }
}

fn render_bar(
    label: &str,
    val: f32,
    max: f32,
    unit: &str,
    width: usize,
    color: Color,
) -> Line<'static> {
    let ratio = if max > 0.0 {
        (val / max).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let filled = (ratio * width as f32).round() as usize;
    let empty = width.saturating_sub(filled);
    let bar = format!("{}{}", "█".repeat(filled), "░".repeat(empty));

    Line::from(vec![
        Span::styled(
            format!("{:<8} ", label),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(bar, Style::default().fg(color)),
        Span::raw(format!(" {}", unit)),
    ])
}

#[allow(clippy::too_many_arguments)]
pub fn draw(
    f: &mut Frame,
    graph: &Graph,
    mode: &AppMode,
    search_query: &str,
    fps: f64,
    userland_view: bool,
    show_all_labels: bool,
    overlay_mode: &OverlayMode,
    show_path_panel: bool,
    _terminal_width: u16,
    _terminal_height: u16,
) {
    let diag = graph.compute_diagnostics();

    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(f.size());

    let header_text = format!(
        " ProcVerse  |  Nodes: {}  |  Clusters: {}  |  Edges: {}  |  FPS: {:.0}",
        diag.total_nodes, diag.total_clusters, diag.total_edges, fps
    );
    f.render_widget(
        Paragraph::new(header_text).style(
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        vertical_chunks[0],
    );

    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(75), Constraint::Percentage(25)])
        .split(vertical_chunks[1]);

    // FIX: Exact matching of constraints to widgets rendered to prevent out-of-bounds panics
    let mut right_constraints = vec![Constraint::Length(12)]; // Details
    if show_path_panel {
        right_constraints.push(Constraint::Min(8)); // Path
    }
    right_constraints.push(Constraint::Length(4)); // Debug
    right_constraints.push(Constraint::Min(6)); // Recent Events
    right_constraints.push(Constraint::Length(4)); // Legend

    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(right_constraints)
        .split(content_chunks[1]);

    let cam_x = graph.camera.x;
    let cam_y = graph.camera.y;
    let zoom = graph.camera.zoom;

    let mut ancestors = HashSet::new();
    let mut descendants = HashSet::new();
    if let Some(selected) = graph.selected_idx {
        ancestors = graph.get_ancestors(selected);
        if graph.trace_mode || graph.focus_mode {
            descendants = graph.get_descendants(selected);
        }
    }

    let canvas = Canvas::default()
        .block(
            Block::default()
                .borders(Borders::RIGHT)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .x_bounds([-100.0, 100.0])
        .y_bounds([-100.0, 100.0])
        .paint(|ctx| {
            let chunk_size = 400.0;
            let layers = [
                (0.2, 120, ".", Color::Rgb(40, 40, 40), 0.8),
                (0.5, 40, "·", Color::Rgb(80, 80, 80), 0.4),
                (0.8, 12, "*", Color::Rgb(150, 150, 150), 0.0),
            ];

            for (layer_idx, (parallax, count, symbol, color, min_zoom)) in layers.iter().enumerate()
            {
                if zoom < *min_zoom {
                    continue;
                }

                let min_w = cam_x * parallax - 100.0 / zoom;
                let max_w = cam_x * parallax + 100.0 / zoom;
                let min_h = cam_y * parallax - 100.0 / zoom;
                let max_h = cam_y * parallax + 100.0 / zoom;

                let start_cx = (min_w / chunk_size).floor() as i32;
                let end_cx = ((max_w / chunk_size).ceil() as i32).min(start_cx + 40);
                let start_cy = (min_h / chunk_size).floor() as i32;
                let end_cy = ((max_h / chunk_size).ceil() as i32).min(start_cy + 40);

                for cx in start_cx..=end_cx {
                    for cy in start_cy..=end_cy {
                        let mut hash = 1337u32;
                        hash = hash.wrapping_add((cx as u32).wrapping_mul(73856093));
                        hash = hash.wrapping_add((cy as u32).wrapping_mul(19349663));
                        hash = hash.wrapping_add((layer_idx as u32).wrapping_mul(83492791));

                        for _ in 0..*count {
                            hash = hash.wrapping_mul(1664525).wrapping_add(1013904223);
                            let rx = (hash % 1000) as f32 / 1000.0;
                            hash = hash.wrapping_mul(1664525).wrapping_add(1013904223);
                            let ry = (hash % 1000) as f32 / 1000.0;

                            let star_w_x = (cx as f32 + rx) * chunk_size;
                            let star_w_y = (cy as f32 + ry) * chunk_size;

                            let sx = (star_w_x - cam_x * parallax) * zoom;
                            let sy = (star_w_y - cam_y * parallax) * zoom;

                            if sx >= -100.0 && sx <= 100.0 && sy >= -100.0 && sy <= 100.0 {
                                ctx.print(
                                    sx as f64,
                                    sy as f64,
                                    Span::styled(*symbol, Style::default().fg(*color)),
                                );
                            }
                        }
                    }
                }
            }

            for edge in &graph.edges {
                let n1 = &graph.nodes[edge.from];
                let n2 = &graph.nodes[edge.to];
                if n1.state == NodeState::Dead
                    || n2.state == NodeState::Dead
                    || n1.is_hidden
                    || n2.is_hidden
                {
                    continue;
                }
                if userland_view && (n1.is_kernel_thread || n2.is_kernel_thread) {
                    continue;
                }

                let is_path = if let Some(sel) = graph.selected_idx {
                    (edge.from == sel && ancestors.contains(&edge.to))
                        || (edge.to == sel && ancestors.contains(&edge.from))
                        || (ancestors.contains(&edge.from) && ancestors.contains(&edge.to))
                        || ((graph.trace_mode || graph.focus_mode)
                            && descendants.contains(&edge.to)
                            && (descendants.contains(&edge.from) || edge.from == sel))
                } else {
                    false
                };

                let mut intensity = 150_f32;
                if (graph.trace_mode || graph.focus_mode) && !is_path {
                    intensity *= 0.15;
                } else if !is_path {
                    intensity *= 0.4;
                } else {
                    intensity = 255_f32;
                }

                if n1.state == NodeState::Dying {
                    intensity *= n1.death_timer.max(0.0);
                }
                if n2.state == NodeState::Dying {
                    intensity *= n2.death_timer.max(0.0);
                }

                let sx1 = (n1.pos.x - cam_x) * zoom;
                let sy1 = (n1.pos.y - cam_y) * zoom;
                let sx2 = (n2.pos.x - cam_x) * zoom;
                let sy2 = (n2.pos.y - cam_y) * zoom;

                ctx.draw(&CanvasLine {
                    x1: sx1 as f64,
                    y1: sy1 as f64,
                    x2: sx2 as f64,
                    y2: sy2 as f64,
                    color: Color::Rgb(intensity as u8, intensity as u8, intensity as u8),
                });
            }

            for (i, node) in graph.nodes.iter().enumerate() {
                if node.state == NodeState::Dead || node.is_hidden {
                    continue;
                }
                if userland_view && node.is_kernel_thread {
                    continue;
                }

                let sx = (node.pos.x - cam_x) * zoom;
                let sy = (node.pos.y - cam_y) * zoom;

                let is_selected = Some(i) == graph.selected_idx;
                let is_ancestor = ancestors.contains(&i);
                let is_descendant = descendants.contains(&i);
                let in_path = is_selected
                    || is_ancestor
                    || ((graph.trace_mode || graph.focus_mode) && is_descendant);
                let is_search_match = *mode == AppMode::Search
                    && !search_query.is_empty()
                    && node
                        .name
                        .to_lowercase()
                        .contains(&search_query.to_lowercase());

                let mut color_factor = 1.0;
                if graph.trace_mode || graph.focus_mode {
                    if !(in_path || is_descendant) {
                        color_factor = 0.15;
                    }
                } else if !in_path {
                    color_factor = 0.5;
                }

                if node.state == NodeState::Dying {
                    color_factor *= node.death_timer.max(0.0);
                }

                let mut final_color = adjust_color(node.color, color_factor);

                if node.state == NodeState::Spawning {
                    if (node.spawn_timer * 30.0).sin() > 0.0 {
                        final_color = adjust_color(Color::White, color_factor);
                    }
                }

                if is_search_match {
                    final_color = Color::White;
                }

                let base_size = match node.importance {
                    Importance::High => 2,
                    Importance::Medium => 1,
                    Importance::Low => 0,
                };

                let overlay_bonus = match overlay_mode {
                    OverlayMode::Normal => 0.0,
                    OverlayMode::Memory => {
                        (node.memory as f32 / graph.max_memory.max(1) as f32) * 5.0
                    }
                    OverlayMode::Threads => {
                        (node.threads as f32 / graph.max_threads.max(1) as f32) * 5.0
                    }
                    OverlayMode::Cpu => (node.cpu_usage / graph.max_cpu.max(0.01)) * 5.0,
                };

                let total_size = (base_size as f32 + overlay_bonus).round() as i32;
                let r = total_size.min(4);

                let mut coords = Vec::new();
                if node.is_cluster {
                    let cr = r.max(1);
                    for dx in -cr..=cr {
                        coords.push(((sx + dx as f32) as f64, (sy - cr as f32) as f64));
                        coords.push(((sx + dx as f32) as f64, (sy + cr as f32) as f64));
                    }
                    for dy in -cr + 1..cr {
                        coords.push(((sx - cr as f32) as f64, (sy + dy as f32) as f64));
                        coords.push(((sx + cr as f32) as f64, (sy + dy as f32) as f64));
                    }
                } else {
                    for dx in -r..=r {
                        for dy in -r..=r {
                            if dx * dx + dy * dy <= r * r + r {
                                coords.push(((sx + dx as f32) as f64, (sy + dy as f32) as f64));
                            }
                        }
                    }
                }

                ctx.draw(&Points {
                    coords: &coords,
                    color: final_color,
                });

                let mut should_draw_label =
                    show_all_labels || is_selected || is_ancestor || is_search_match;
                if !should_draw_label {
                    if node.importance == Importance::High || node.is_cluster {
                        should_draw_label = true;
                    } else if node.importance == Importance::Medium
                        && (is_descendant || is_ancestor)
                    {
                        should_draw_label = true;
                    }
                }

                if should_draw_label {
                    let text = if node.is_cluster {
                        format!("[{} x{}]", node.name.to_uppercase(), node.cluster_count)
                    } else {
                        node.name.clone()
                    };

                    let (display_text, style) = if is_selected {
                        (
                            format!("▶ {} ◀", text),
                            Style::default()
                                .bg(Color::Rgb(60, 60, 60))
                                .fg(Color::White)
                                .add_modifier(Modifier::BOLD),
                        )
                    } else if is_search_match {
                        (
                            text,
                            Style::default()
                                .fg(final_color)
                                .add_modifier(Modifier::BOLD),
                        )
                    } else {
                        (text, Style::default().fg(final_color))
                    };

                    let label_x = sx as f64 - (display_text.len() as f64 * 0.4);
                    let label_y = sy as f64 + (r as f64) + 2.0;

                    ctx.print(label_x, label_y, Span::styled(display_text, style));
                }
            }
        });

    f.render_widget(canvas, content_chunks[0]);

    if *mode == AppMode::Search {
        let search_area = Rect::new(
            content_chunks[0]
                .x
                .saturating_add(content_chunks[0].width / 2)
                .saturating_sub(20),
            content_chunks[0].y.saturating_add(2),
            40,
            3,
        );
        f.render_widget(Clear, search_area);
        let search_block = Paragraph::new(format!("> {}_", search_query))
            .block(Block::default().borders(Borders::ALL).title(" Search "));
        f.render_widget(search_block, search_area);
    }

    let mut r_idx = 0;
    let details_block = Block::default().borders(Borders::NONE);

    // 1. Process Details (Always)
    if let Some(idx) = graph.selected_idx {
        if let Some(node) = graph.nodes.get(idx) {
            if node.is_cluster {
                let text = vec![
                    Line::from(Span::styled(
                        format!("[{} x{}]", node.name.to_uppercase(), node.cluster_count),
                        Style::default().add_modifier(Modifier::BOLD).fg(node.color),
                    )),
                    Line::raw(""),
                    Line::from(vec![
                        Span::styled(
                            format!("{:<8} ", "Type"),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::raw("Cluster"),
                    ]),
                    Line::from(vec![
                        Span::styled(
                            format!("{:<8} ", "Expanded"),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::raw(if node.cluster_expanded { "Yes" } else { "No" }),
                    ]),
                    Line::raw(""),
                    render_bar(
                        "Memory",
                        node.memory as f32,
                        graph.max_memory as f32,
                        &format!("{} MB", node.memory / 1024 / 1024),
                        10,
                        Color::Cyan,
                    ),
                    render_bar(
                        "Threads",
                        node.threads as f32,
                        graph.max_threads as f32,
                        &format!("{}", node.threads),
                        10,
                        Color::Green,
                    ),
                ];
                f.render_widget(
                    Paragraph::new(text).block(details_block),
                    right_chunks[r_idx],
                );
            } else {
                let title_style = Style::default().add_modifier(Modifier::BOLD).fg(node.color);
                let text = vec![
                    Line::from(Span::styled(node.name.clone(), title_style)),
                    Line::raw(""),
                    Line::from(vec![
                        Span::styled(
                            format!("{:<8} ", "PID"),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::raw(format!("{}", node.pid)),
                    ]),
                    Line::from(vec![
                        Span::styled(
                            format!("{:<8} ", "PPID"),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::raw(format!("{}", node.ppid)),
                    ]),
                    Line::raw(""),
                    render_bar(
                        "Memory",
                        node.memory as f32,
                        graph.max_memory as f32,
                        &format!("{} MB", node.memory / 1024 / 1024),
                        10,
                        Color::Cyan,
                    ),
                    render_bar(
                        "Threads",
                        node.threads as f32,
                        graph.max_threads as f32,
                        &format!("{}", node.threads),
                        10,
                        Color::Green,
                    ),
                    render_bar(
                        "CPU",
                        node.cpu_usage,
                        graph.max_cpu,
                        &format!("{:.1}%", node.cpu_usage),
                        10,
                        Color::Yellow,
                    ),
                    Line::raw(""),
                    Line::from(Span::styled(
                        node.exe.clone(),
                        Style::default().fg(Color::DarkGray),
                    )),
                ];
                f.render_widget(
                    Paragraph::new(text).block(details_block),
                    right_chunks[r_idx],
                );
            }
        }
    }
    r_idx += 1;

    // 2. Process Path (Conditional)
    if show_path_panel {
        let mut path_lines = Vec::new();
        if let Some(idx) = graph.selected_idx {
            let path = graph.get_process_path(idx);
            for (depth, &n_idx) in path.iter().enumerate() {
                let node = &graph.nodes[n_idx];
                let name = if node.is_cluster {
                    format!("[{} x{}]", node.name.to_uppercase(), node.cluster_count)
                } else {
                    node.name.clone()
                };

                if depth > 0 {
                    path_lines.push(Line::from(Span::styled(
                        "  ↓",
                        Style::default().fg(Color::DarkGray),
                    )));
                }
                path_lines.push(Line::from(Span::styled(
                    name,
                    Style::default().fg(node.color),
                )));
            }
        }
        f.render_widget(
            Paragraph::new(path_lines).block(
                Block::default()
                    .borders(Borders::TOP)
                    .border_style(Style::default().fg(Color::DarkGray)),
            ),
            right_chunks[r_idx],
        );
        r_idx += 1;
    }

    // 3. Debug/Diagnostics (Always)
    let diag_text = vec![
        Line::from(format!(
            "Cam X: {:.1}  |  Cam Y: {:.1}  |  Zoom: {:.2}",
            cam_x, cam_y, zoom
        )),
        Line::from(format!("Avg Radius: {:.1}", diag.avg_dist)),
    ];
    f.render_widget(
        Paragraph::new(diag_text).block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(" Debug "),
        ),
        right_chunks[r_idx],
    );
    r_idx += 1;

    // 4. Recent Events (Always)
    let mut event_lines = Vec::new();
    for ev in &graph.events {
        let prefix = match ev.event_type {
            EventType::Spawn => Span::styled("+ ", Style::default().fg(Color::Green)),
            EventType::Exit => Span::styled("- ", Style::default().fg(Color::Red)),
        };
        event_lines.push(Line::from(vec![prefix, Span::raw(ev.name.clone())]));
    }
    f.render_widget(
        Paragraph::new(event_lines).block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(" Recent "),
        ),
        right_chunks[r_idx],
    );
    r_idx += 1;

    // 5. Legend (Always)
    let legend_text = vec![
        Line::from(vec![
            Span::styled("■ ", Style::default().fg(Color::Yellow)),
            Span::raw("System   "),
            Span::styled("■ ", Style::default().fg(Color::Green)),
            Span::raw("Shell   "),
            Span::styled("■ ", Style::default().fg(Color::Cyan)),
            Span::raw("Editor"),
        ]),
        Line::from(vec![
            Span::styled("■ ", Style::default().fg(Color::Red)),
            Span::raw("Browser  "),
            Span::styled("■ ", Style::default().fg(Color::Blue)),
            Span::raw("Docker  "),
            Span::styled("■ ", Style::default().fg(Color::Magenta)),
            Span::raw("Cluster"),
        ]),
    ];
    f.render_widget(
        Paragraph::new(legend_text).block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(Color::DarkGray)),
        ),
        right_chunks[r_idx],
    );

    let view_str = if userland_view {
        "USERLAND"
    } else {
        "FULL SYS"
    };
    let overlay_str = match overlay_mode {
        OverlayMode::Normal => "NORMAL",
        OverlayMode::Memory => "MEMORY",
        OverlayMode::Threads => "THREADS",
        OverlayMode::Cpu => "CPU",
    };
    let trace_str = if graph.trace_mode {
        "TRACE ON"
    } else {
        "TRACE OFF"
    };
    let mode_str = match mode {
        AppMode::Normal => {
            if graph.focus_mode {
                "FOCUS ON"
            } else {
                "NORMAL"
            }
        }
        AppMode::Search => "SEARCH",
    };

    let status_text = format!(" [{}] [{}] [{}] [{}]  |  / Search | P Path | R Res | D Trace | Space Expand | Mouse/Arrows Nav | Q Quit",
        mode_str, view_str, overlay_str, trace_str);

    f.render_widget(
        Paragraph::new(status_text).style(Style::default().fg(Color::DarkGray)),
        vertical_chunks[2],
    );
}
