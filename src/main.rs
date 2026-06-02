mod graph;
mod physics;
mod process_scanner;
mod renderer;

use crossterm::{
    event::{self, Event, KeyCode, MouseEventKind, MouseButton},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use graph::{Graph, NodeState};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{
    io,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

#[derive(PartialEq)]
pub enum AppMode {
    Normal,
    Search,
}

fn main() -> anyhow::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, event::EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        process_scanner::run_scanner(tx);
    });

    let mut graph = Graph::new();
    let mut tick = 0.0;
    
    let mut mode = AppMode::Normal;
    let mut search_query = String::new();
    
    let mut userland_view = true;
    let mut show_all_labels = false;

    let mut last_frame = Instant::now();
    let mut fps = 0.0;
    let mut is_first_snapshot = true;

    let mut is_dragging = false;
    let mut last_mouse_col = 0;
    let mut last_mouse_row = 0;

    loop {
        let now = Instant::now();
        let dt_frame = now.duration_since(last_frame).as_secs_f64();
        last_frame = now;
        if dt_frame > 0.0 {
            fps = (fps * 0.9) + ((1.0 / dt_frame) * 0.1);
        }

        if let Ok(snapshot) = rx.try_recv() {
            graph.update_from_snapshot(&snapshot);

            if is_first_snapshot {
                for _ in 0..150 {
                    physics::step(&mut graph, 0.05, tick);
                    tick += 1.0;
                }
                graph.auto_fit_camera();
                is_first_snapshot = false;
            }
        }

        if !is_first_snapshot {
            physics::step(&mut graph, 0.05, tick);
            tick += 1.0;
        }

        let terminal_size = terminal.size()?;
        terminal.draw(|f| renderer::draw(f, &graph, &mode, &search_query, fps, userland_view, show_all_labels))?;

        if event::poll(Duration::from_millis(16))? {
            match event::read()? {
                Event::Key(key) => {
                    if mode == AppMode::Search {
                        match key.code {
                            KeyCode::Esc => {
                                mode = AppMode::Normal;
                                search_query.clear();
                            }
                            KeyCode::Enter => {
                                if let Some(idx) = graph.nodes.iter().position(|n| {
                                    n.name.to_lowercase().contains(&search_query.to_lowercase()) 
                                    && n.state != NodeState::Dead
                                    && !n.is_hidden
                                    && !(userland_view && n.is_kernel_thread)
                                }) {
                                    graph.selected_idx = Some(idx);
                                }
                                mode = AppMode::Normal;
                                search_query.clear();
                            }
                            KeyCode::Backspace => { search_query.pop(); }
                            KeyCode::Char(c) => { search_query.push(c); }
                            _ => {}
                        }
                    } else {
                        match key.code {
                            KeyCode::Char('q') => break,
                            KeyCode::Char('/') => {
                                mode = AppMode::Search;
                                search_query.clear();
                            }
                            KeyCode::Char(' ') => {
                                if let Some(idx) = graph.selected_idx {
                                    if graph.nodes[idx].is_cluster {
                                        graph.nodes[idx].cluster_expanded = !graph.nodes[idx].cluster_expanded;
                                    }
                                }
                            }
                            KeyCode::Char('u') | KeyCode::Char('U') => {
                                userland_view = !userland_view;
                                if userland_view {
                                    if let Some(idx) = graph.selected_idx {
                                        if graph.nodes[idx].is_kernel_thread {
                                            graph.selected_idx = graph.nodes.iter().position(|n| n.pid == 1 && n.state != NodeState::Dead).or(Some(0));
                                        }
                                    }
                                }
                            }
                            KeyCode::Char('l') | KeyCode::Char('L') => { show_all_labels = !show_all_labels; }
                            KeyCode::Enter => graph.focus_mode = !graph.focus_mode,
                            KeyCode::Tab => {
                                if !graph.nodes.is_empty() {
                                    let start = graph.selected_idx.unwrap_or(0);
                                    let mut next = (start + 1) % graph.nodes.len();
                                    while (graph.nodes[next].state == NodeState::Dead || graph.nodes[next].is_hidden || (userland_view && graph.nodes[next].is_kernel_thread)) && next != start {
                                        next = (next + 1) % graph.nodes.len();
                                    }
                                    graph.selected_idx = Some(next);
                                }
                            }
                            KeyCode::Up => graph.camera.y += 10.0 / graph.camera.zoom.max(0.1),
                            KeyCode::Down => graph.camera.y -= 10.0 / graph.camera.zoom.max(0.1),
                            KeyCode::Right => graph.camera.x += 10.0 / graph.camera.zoom.max(0.1),
                            KeyCode::Left => graph.camera.x -= 10.0 / graph.camera.zoom.max(0.1),
                            KeyCode::Char('+') => graph.camera.zoom *= 1.2,
                            KeyCode::Char('-') => graph.camera.zoom /= 1.2,
                            KeyCode::Char('f') => graph.center_on_selection(),
                            KeyCode::Home => graph.auto_fit_camera(),
                            _ => {}
                        }
                    }
                }
                Event::Mouse(mouse_event) => {
                    let canvas_width = (terminal_size.width as f32 * 0.75).floor();
                    let canvas_height = terminal_size.height.saturating_sub(1) as f32;
                    let col = mouse_event.column as f32;
                    let row = mouse_event.row as f32;

                    if col <= canvas_width && row <= canvas_height {
                        let math_x = (col / canvas_width) * 200.0 - 100.0;
                        let math_y = 100.0 - (row / canvas_height) * 200.0;
                        let world_x = (math_x / graph.camera.zoom) + graph.camera.x;
                        let world_y = (math_y / graph.camera.zoom) + graph.camera.y;

                        match mouse_event.kind {
                            MouseEventKind::ScrollUp => { graph.camera.zoom *= 1.2; }
                            MouseEventKind::ScrollDown => { graph.camera.zoom /= 1.2; }
                            MouseEventKind::Down(MouseButton::Left) => {
                                is_dragging = true;
                                last_mouse_col = mouse_event.column;
                                last_mouse_row = mouse_event.row;
                                graph.select_nearest(world_x, world_y, userland_view);
                            }
                            MouseEventKind::Up(MouseButton::Left) => { is_dragging = false; }
                            MouseEventKind::Drag(MouseButton::Left) => {
                                if is_dragging {
                                    let dx = mouse_event.column as f32 - last_mouse_col as f32;
                                    let dy = mouse_event.row as f32 - last_mouse_row as f32;
                                    graph.camera.x -= dx * (200.0 / canvas_width) / graph.camera.zoom;
                                    graph.camera.y += dy * (200.0 / canvas_height) / graph.camera.zoom;
                                    last_mouse_col = mouse_event.column;
                                    last_mouse_row = mouse_event.row;
                                }
                            }
                            MouseEventKind::Down(MouseButton::Middle) => { graph.auto_fit_camera(); }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, event::DisableMouseCapture)?;
    Ok(())
}