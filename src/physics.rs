use crate::graph::{Graph, NodeState};
use glam::Vec2;

pub fn step(graph: &mut Graph, dt: f32, tick: f32) {
    let k = 20.0_f32;
    let damping = 0.70_f32;
    let num_nodes = graph.nodes.len();

    if num_nodes == 0 {
        return;
    }

    let mut forces = vec![Vec2::ZERO; num_nodes];

    for i in 0..num_nodes {
        if graph.nodes[i].state == NodeState::Dead || graph.nodes[i].is_hidden {
            continue;
        }

        // Global gravity to prevent any math anomalies or disconnected orphans from escaping bounds
        let dist_to_center = graph.nodes[i].pos.length();
        if dist_to_center > 10.0 {
            forces[i] += (graph.nodes[i].pos / dist_to_center) * -0.5;
        }

        for j in 0..num_nodes {
            if i == j || graph.nodes[j].state == NodeState::Dead || graph.nodes[j].is_hidden {
                continue;
            }

            let mut dir = graph.nodes[i].pos - graph.nodes[j].pos;
            let mut dist = dir.length();
            if dist < 0.1 {
                let hash_i = (graph.nodes[i].pid % 11) as f32;
                let hash_j = (graph.nodes[j].pid % 13) as f32;
                dir = Vec2::new(hash_i - 5.0, hash_j - 6.0);
                if dir.length() < 0.01 {
                    dir = Vec2::new(1.0, 0.0);
                }
                dist = dir.length().max(0.1);
            }

            let radius_i = graph.nodes[i].name.len() as f32 * 0.6;
            let radius_j = graph.nodes[j].name.len() as f32 * 0.6;
            let safe_dist = radius_i + radius_j + 2.0;

            let force_mag = if dist < safe_dist {
                (k * k) / dist * 5.0
            } else {
                (k * k) / dist
            };

            forces[i] += (dir / dist) * force_mag;
        }
    }

    for edge in &graph.edges {
        let i = edge.from;
        let j = edge.to;
        if graph.nodes[i].state == NodeState::Dead
            || graph.nodes[i].is_hidden
            || graph.nodes[j].state == NodeState::Dead
            || graph.nodes[j].is_hidden
        {
            continue;
        }

        let mut dir = graph.nodes[j].pos - graph.nodes[i].pos;
        let mut dist = dir.length();
        if dist < 0.1 {
            dir = Vec2::new(1.0, 0.0);
            dist = 0.1;
        }

        let scaled_dist = dist.min(300.0);
        let force_mag = (scaled_dist * scaled_dist) / k;
        let force_vec = (dir / dist) * force_mag;

        forces[i] += force_vec;
        forces[j] -= force_vec;
    }

    for i in 0..num_nodes {
        if graph.nodes[i].state == NodeState::Dead || graph.nodes[i].is_hidden {
            continue;
        }

        if graph.nodes[i].depth > 0 {
            let center_dir = graph.nodes[i].pos;
            let current_radius = center_dir.length().max(0.1);
            let desired_radius = graph.nodes[i].depth as f32 * 20.0;
            let radial_error = current_radius - desired_radius;
            forces[i] += (center_dir / current_radius) * -radial_error * 0.8;
        }

        forces[i] += Vec2::new(
            (tick * 0.05 + i as f32).sin(),
            (tick * 0.05 + i as f32 * 1.5).cos(),
        ) * 0.02;
    }

    for i in 0..num_nodes {
        let node = &mut graph.nodes[i];
        if node.state == NodeState::Dead || node.is_hidden {
            continue;
        }

        if node.state == NodeState::Spawning {
            node.spawn_timer -= dt;
            if node.spawn_timer <= 0.0 {
                node.state = NodeState::Alive;
            }
        } else if node.state == NodeState::Dying {
            node.death_timer -= dt;
            if node.death_timer <= 0.0 {
                node.state = NodeState::Dead;
            }
        }

        if node.fixed {
            node.vel = Vec2::ZERO;
            node.pos = Vec2::ZERO;
            continue;
        }

        node.vel = (node.vel + forces[i] * dt) * damping;

        let max_vel = 100.0;
        if node.vel.length() > max_vel {
            node.vel = node.vel.normalize() * max_vel;
        }

        node.pos += node.vel * dt;

        if node.pos.x.is_nan() || node.pos.y.is_nan() {
            node.pos = Vec2::ZERO;
            node.vel = Vec2::ZERO;
        }
    }
}
