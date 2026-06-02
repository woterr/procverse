use crate::graph::{EventType, Graph, NodeState, Importance};
use crate::AppMode;
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

#[allow(clippy::too_many_arguments)]
pub fn draw(
    f: &mut Frame,
    graph: &Graph,
    mode: &AppMode,
    search_query: &str,
    fps: f64,
    userland_view: bool,
    show_all_labels: bool,
) {
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(f.size());

    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(75), Constraint::Percentage(25)])
        .split(main_layout[0]);

    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(14), Constraint::Min(14), Constraint::Percentage(100)])
        .split(content_chunks[1]);

    let cam_x = graph.camera.x;
    let cam_y = graph.camera.y;
    let zoom = graph.camera.zoom;

    let mut ancestors = HashSet::new();
    let mut descendants = HashSet::new();
    if let Some(selected) = graph.selected_idx {
        ancestors = graph.get_ancestors(selected);
        if graph.focus_mode {
            descendants = graph.get_descendants(selected);
        }
    }

    let canvas = Canvas::default()
        .block(Block::default().borders(Borders::ALL).title(" ProcVerse "))
        .x_bounds([-100.0, 100.0])
        .y_bounds([-100.0, 100.0])
        .paint(|ctx| {
            for star in &graph.stars {
                let sx = (star.x - cam_x) * zoom;
                let sy = (star.y - cam_y) * zoom;
                ctx.draw(&Points {
                    coords: &[(sx as f64, sy as f64)],
                    color: Color::Rgb(40, 40, 40),
                });
            }

            for edge in &graph.edges {
                let n1 = &graph.nodes[edge.from];
                let n2 = &graph.nodes[edge.to];
                if n1.state == NodeState::Dead || n2.state == NodeState::Dead || n1.is_hidden || n2.is_hidden { continue; }
                if userland_view && (n1.is_kernel_thread || n2.is_kernel_thread) { continue; }
                
                let is_path = if let Some(sel) = graph.selected_idx {
                    (edge.from == sel && ancestors.contains(&edge.to)) || 
                    (edge.to == sel && ancestors.contains(&edge.from)) ||
                    (ancestors.contains(&edge.from) && ancestors.contains(&edge.to)) ||
                    (graph.focus_mode && descendants.contains(&edge.to) && (descendants.contains(&edge.from) || edge.from == sel))
                } else {
                    false
                };

                let mut intensity = 150_f32;
                if graph.focus_mode && !is_path {
                    intensity *= 0.15;
                } else if !is_path {
                    intensity *= 0.4;
                } else {
                    intensity = 255_f32;
                }

                if n1.state == NodeState::Dying { intensity *= n1.death_timer.max(0.0); }
                if n2.state == NodeState::Dying { intensity *= n2.death_timer.max(0.0); }

                let sx1 = (n1.pos.x - cam_x) * zoom;
                let sy1 = (n1.pos.y - cam_y) * zoom;
                let sx2 = (n2.pos.x - cam_x) * zoom;
                let sy2 = (n2.pos.y - cam_y) * zoom;

                ctx.draw(&CanvasLine {
                    x1: sx1 as f64, y1: sy1 as f64, x2: sx2 as f64, y2: sy2 as f64,
                    color: Color::Rgb(intensity as u8, intensity as u8, intensity as u8),
                });
            }

            for (i, node) in graph.nodes.iter().enumerate() {
                if node.state == NodeState::Dead || node.is_hidden { continue; }
                if userland_view && node.is_kernel_thread { continue; }

                let sx = (node.pos.x - cam_x) * zoom;
                let sy = (node.pos.y - cam_y) * zoom;

                let is_selected = Some(i) == graph.selected_idx;
                let is_ancestor = ancestors.contains(&i);
                let is_descendant = descendants.contains(&i);
                let in_path = is_selected || is_ancestor || (graph.focus_mode && is_descendant);
                let is_search_match = *mode == AppMode::Search && !search_query.is_empty() && node.name.to_lowercase().contains(&search_query.to_lowercase());

                let mut color_factor = 1.0;
                if graph.focus_mode {
                    if !(in_path || is_descendant) { color_factor = 0.2; }
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

                if node.is_cluster {
                    ctx.draw(&Points {
                        coords: &[
                            (sx as f64 - 1.0, sy as f64 - 1.0), (sx as f64, sy as f64 - 1.0), (sx as f64 + 1.0, sy as f64 - 1.0),
                            (sx as f64 - 1.0, sy as f64),                                     (sx as f64 + 1.0, sy as f64),
                            (sx as f64 - 1.0, sy as f64 + 1.0), (sx as f64, sy as f64 + 1.0), (sx as f64 + 1.0, sy as f64 + 1.0),
                        ], color: final_color,
                    });
                } else if is_selected || node.importance == Importance::High {
                    ctx.draw(&Points {
                        coords: &[
                            (sx as f64, sy as f64), ((sx + 1.0) as f64, sy as f64), ((sx - 1.0) as f64, sy as f64),
                            (sx as f64, (sy + 1.0) as f64), ((sx + 1.0) as f64, (sy + 1.0) as f64), ((sx - 1.0) as f64, (sy + 1.0) as f64),
                            (sx as f64, (sy - 1.0) as f64), ((sx + 1.0) as f64, (sy - 1.0) as f64), ((sx - 1.0) as f64, (sy - 1.0) as f64),
                        ], color: final_color,
                    });
                } else if node.importance == Importance::Medium {
                    ctx.draw(&Points {
                        coords: &[
                            (sx as f64, sy as f64), ((sx + 1.0) as f64, sy as f64),
                            (sx as f64, (sy - 1.0) as f64), ((sx + 1.0) as f64, (sy - 1.0) as f64),
                        ], color: final_color,
                    });
                } else {
                    ctx.draw(&Points { coords: &[(sx as f64, sy as f64)], color: final_color });
                }

                let mut should_draw_label = show_all_labels || is_selected || is_ancestor || is_search_match;
                if !should_draw_label {
                    if node.importance == Importance::High || node.is_cluster {
                        should_draw_label = true;
                    } else if node.importance == Importance::Medium && (is_descendant || is_ancestor) {
                        should_draw_label = true;
                    }
                }
                
                if should_draw_label {
                    let text = if node.is_cluster {
                        format!("[{} x{}]", node.name.to_uppercase(), node.cluster_count)
                    } else {
                        node.name.clone()
                    };
                    
                    let label_x = sx as f64 - (text.len() as f64 * 0.4);
                    let label_y = sy as f64 + 3.0;

                    let style = if is_selected || is_search_match {
                        Style::default().fg(final_color).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(final_color)
                    };

                    ctx.print(label_x, label_y, Span::styled(text, style));
                }
            }
        });

    f.render_widget(canvas, content_chunks[0]);

    if *mode == AppMode::Search {
        let search_area = Rect::new(
            content_chunks[0].x.saturating_add(content_chunks[0].width / 2).saturating_sub(20),
            content_chunks[0].y.saturating_add(2),
            40,
            3,
        );
        f.render_widget(Clear, search_area);
        let search_block = Paragraph::new(format!("> {}_", search_query))
            .block(Block::default().borders(Borders::ALL).title(" Search "));
        f.render_widget(search_block, search_area);
    }

    let stats_area = Rect::new(
        content_chunks[0].x.saturating_add(content_chunks[0].width).saturating_sub(16),
        content_chunks[0].y.saturating_add(1),
        15,
        3,
    );
    let stats_text = vec![Line::from(format!("FPS: {:.0}", fps))];
    f.render_widget(Clear, stats_area);
    f.render_widget(Paragraph::new(stats_text).block(Block::default().borders(Borders::ALL)), stats_area);

    if let Some(idx) = graph.selected_idx {
        if let Some(node) = graph.nodes.get(idx) {
            if node.is_cluster {
                let text = vec![
                    Line::from(vec![Span::styled("Type: ", Style::default().fg(Color::DarkGray)), Span::raw("Cluster")]),
                    Line::from(vec![Span::styled("Name: ", Style::default().fg(Color::DarkGray)), Span::raw(node.name.clone())]),
                    Line::from(vec![Span::styled("Processes: ", Style::default().fg(Color::DarkGray)), Span::raw(format!("{}", node.cluster_count))]),
                    Line::from(vec![Span::styled("Expanded: ", Style::default().fg(Color::DarkGray)), Span::raw(if node.cluster_expanded { "Yes" } else { "No" })]),
                    Line::raw(""),
                    Line::from(vec![Span::styled("Status: ", Style::default().fg(Color::DarkGray)), Span::raw("Active")]),
                ];
                f.render_widget(Paragraph::new(text).block(Block::default().borders(Borders::ALL).title(" Cluster Details ")), right_chunks[0]);
            } else {
                let children_count = graph.get_children_count(idx);
                use chrono::{TimeZone, Local};
                let start_time_str = Local.timestamp_opt(node.start_time as i64, 0).unwrap().format("%H:%M:%S").to_string();

                let text = vec![
                    Line::from(vec![Span::styled("Type: ", Style::default().fg(Color::DarkGray)), Span::raw("Process")]),
                    Line::from(vec![Span::styled("PID: ", Style::default().fg(Color::DarkGray)), Span::raw(format!("{}", node.pid))]),
                    Line::from(vec![Span::styled("PPID: ", Style::default().fg(Color::DarkGray)), Span::raw(format!("{}", node.ppid))]),
                    Line::from(vec![Span::styled("Name: ", Style::default().fg(Color::DarkGray)), Span::raw(node.name.clone())]),
                    Line::from(vec![Span::styled("Exe: ", Style::default().fg(Color::DarkGray)), Span::raw(node.exe.clone())]),
                    Line::from(vec![Span::styled("Threads: ", Style::default().fg(Color::DarkGray)), Span::raw(format!("{}", node.threads))]),
                    Line::from(vec![Span::styled("Memory: ", Style::default().fg(Color::DarkGray)), Span::raw(format!("{} MB", node.memory / 1024 / 1024))]),
                    Line::from(vec![Span::styled("Children: ", Style::default().fg(Color::DarkGray)), Span::raw(format!("{}", children_count))]),
                    Line::from(vec![Span::styled("Started: ", Style::default().fg(Color::DarkGray)), Span::raw(start_time_str)]),
                    Line::raw(""),
                    Line::from(vec![Span::styled("Status: ", Style::default().fg(Color::DarkGray)), Span::raw(
                        match node.state {
                            NodeState::Spawning => "Spawning", NodeState::Alive => "Alive", NodeState::Dying => "Terminating", NodeState::Dead => "Dead",
                        }
                    )]),
                ];
                f.render_widget(Paragraph::new(text).block(Block::default().borders(Borders::ALL).title(" Process Details ")), right_chunks[0]);
            }
        }
    }

    let diag = graph.compute_diagnostics();
    let diag_text = vec![
        Line::from(format!("Nodes: {}", diag.total_nodes)),
        Line::from(format!("Edges: {}", diag.total_edges)),
        Line::raw(""),
        Line::from(format!("Avg Radius: {:.1}", diag.avg_dist)),
        Line::from(format!("Max Radius: {:.1}", diag.max_dist)),
        Line::from(format!("Avg Depth: {:.1}", diag.avg_depth)),
        Line::from(format!("Max Depth: {}", diag.max_depth)),
        Line::raw(""),
        Line::from("Bounds:"),
        Line::from(format!("X: {:.0} -> {:.0}", diag.min_x, diag.max_x)),
        Line::from(format!("Y: {:.0} -> {:.0}", diag.min_y, diag.max_y)),
    ];
    f.render_widget(Paragraph::new(diag_text).block(Block::default().borders(Borders::ALL).title(" Diagnostics ")), right_chunks[1]);

    let mut event_lines = Vec::new();
    for ev in &graph.events {
        let prefix = match ev.event_type {
            EventType::Spawn => Span::styled("+ ", Style::default().fg(Color::Green)),
            EventType::Exit => Span::styled("- ", Style::default().fg(Color::Red)),
        };
        event_lines.push(Line::from(vec![prefix, Span::raw(ev.name.clone())]));
    }
    f.render_widget(Paragraph::new(event_lines).block(Block::default().borders(Borders::ALL).title(" Recent Events ")), right_chunks[2]);

    let view_str = if userland_view { "USERLAND" } else { "FULL SYSTEM" };
    let label_str = if show_all_labels { "ALL" } else { "CONTEXT" };
    let mode_str = match mode {
        AppMode::Normal => if graph.focus_mode { "NORMAL (FOCUS ON)" } else { "NORMAL" },
        AppMode::Search => "SEARCH",
    };
    let status_text = format!("Mode: {} | View: {} | Labels: {} | SPACE Expand | U/L Toggle | TAB/Mouse Select | F/Home Camera | Q Quit", mode_str, view_str, label_str);
    f.render_widget(Paragraph::new(status_text).style(Style::default().fg(Color::DarkGray)), main_layout[1]);
}