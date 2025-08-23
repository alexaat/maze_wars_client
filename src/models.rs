use crate::preferences::*;
use macroquad::prelude::*;
use macroquad::prelude::{Image, Texture2D, Vec2, Vec3};
use macroquad::{color::Color, texture::RenderTarget};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PlayerStatus {
    Active,
    Disconnent,
    Killed,
    Unknown,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Player {
    pub id: String,
    pub name: String,
    pub position: Position,
    pub score: u32,
    //pub is_active: bool,
    pub player_status: PlayerStatus,
    pub orientation: f32,
    pub current_map: String,
    #[serde(skip_serializing, skip_deserializing)]
    pub mini_map: Vec<Vec<bool>>,
}
impl Player {
    pub fn new() -> Self {
        Player {
            id: Uuid::new_v4().to_string(),
            name: String::from(""),
            position: Position::new(),
            score: 0,
            //is_active: true,
            player_status: PlayerStatus::Active,
            orientation: 0.0,
            current_map: String::from(""),
            mini_map: vec![],
        }
    }
    //function that when player is used as enemy calculate if it was hit by other player
    pub fn hit(&self, origin: Vec3, direction: Vec3) -> Option<Hit> {
        let hittable = Hittable::Enemy(self.clone());
        //calculate components

        let a = direction.dot(direction);
        if a.abs() < MIN_SHOT_HIT_TIME {
            return None;
        }
        let position = vec3(self.position.x, PLAYER_HEIGHT, self.position.z);
        let c_o = position - origin;
        let b = -2.0 * (c_o.dot(direction));
        let c = c_o.dot(c_o) - ENEMY_RADIUS * ENEMY_RADIUS;
        let discriminant = b * b - 4.0 * a * c;
        if discriminant < 0.0 {
            return None;
        }
        let discriminant_root = discriminant.sqrt();
        if discriminant_root == 0.0 {
            let t = -b / (2.0 * a);
            if t > 0.0 {
                let p = origin + t * direction;
                return Some(Hit { t, p, hittable });
            } else {
                return None;
            }
        }
        let t1 = (-b - discriminant_root) / (2.0 * a);
        let t2 = (-b + discriminant_root) / (2.0 * a);
        if t1 < 0.0 && t2 < 0.0 {
            return None;
        }
        let t = if t1 > 0.0 {
            if t2 > 0.0 {
                f32::min(t1, t2)
            } else {
                t1
            }
        } else {
            t2
        };
        let p = origin + t * direction;
        Some(Hit { t, p, hittable })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Position {
    pub x: f32,
    pub z: f32,
}
impl Position {
    pub fn new() -> Self {
        Position { x: 0.0, z: 0.0 }
    }
    pub fn build(x: f32, z: f32) -> Self {
        let mut pos = Position::new();
        pos.x = x;
        pos.z = z;
        pos
    }
}

#[derive(Debug)]
pub enum Status {
    EnterIP,
    EnterName,
    StartServerListener,
    Run,
}

#[derive(Debug)]
pub struct MiniMapConfig {
    pub cell_width: f32,
    pub cell_height: f32,
    pub cell_color: Color,
    pub horizontal_offset: u32,
    pub vertical_offset: u32,
}
impl MiniMapConfig {
    pub fn new(
        mini_map: &Vec<Vec<bool>>,
        mini_map_width: u32,
        mini_map_height: u32,
        horizontal_offset: u32,
        vertical_offset: u32,
        cell_color: Color,
    ) -> Self {
        let h = mini_map.len();
        let w = mini_map[0].len();
        let cell_width = mini_map_width as f32 / w as f32;
        let cell_height = mini_map_height as f32 / h as f32;
        MiniMapConfig {
            cell_width,
            cell_height,
            horizontal_offset,
            vertical_offset,
            cell_color,
        }
    }
}

pub struct GameParams {
    pub wall_texture: Texture2D,
    pub sky_texture: Texture2D,
    pub arrow_texture: Texture2D,
    pub eye_texture: Image,
    pub mini_map_config: MiniMapConfig,
    pub render_target: RenderTarget,
    pub yaw: f32,
    pub pitch: f32,
    pub front: Vec3,
    pub right: Vec3,
    pub position: Vec3,
    pub last_mouse_position: Vec2,
    pub mini_map: Vec<Vec<bool>>,
    pub world_up: Vec3,
    pub shots: Vec<Shot>,
    pub hittables: Arc<Mutex<Vec<Hittable>>>,
}

#[derive(Debug, Clone)]
pub struct Shot {
    pub start: Vec3,
    pub end: Vec3,
    pub time_out: i32,
    pub color: Color,
}

#[derive(Debug, Clone)]
pub struct Shield {
    pub q: Vec3, //origin
    pub u: Vec3, //horizontal vector
    pub v: Vec3, //vertical vector
}

impl Shield {
    pub fn new(q: Vec3, u: Vec3, v: Vec3) -> Self {
        Self { q, u, v }
    }

    pub fn hit(&self, origin: Vec3, direction: Vec3) -> Option<Hit> {
        let n = self.u.cross(self.v);
        let denominator = n.dot(direction);
        if denominator.abs() < MIN_SHOT_HIT_TIME {
            return None;
        }
        let t = (n.dot(self.q) - n.dot(origin)) / denominator;
        if t < MIN_SHOT_HIT_TIME || t > MAX_SHOT_HIT_TIME {
            return None;
        }
        let p = origin + direction * t;
        //test for intersection
        let w = p - self.q;
        //projection on u
        let proj_on_u = (w.dot(self.u)) / self.u.length();
        if proj_on_u < 0.0 || proj_on_u > self.u.length() {
            return None;
        }
        let proj_on_v = (w.dot(self.v)) / self.v.length();
        if proj_on_v < 0.0 || proj_on_v > self.v.length() {
            return None;
        }
        let hittable = Hittable::Wall(self.clone());
        Some(Hit { t, p, hittable })
    }
}

#[derive(Debug)]
pub struct Hit {
    pub t: f32,
    pub p: Vec3,
    pub hittable: Hittable,
}

#[derive(Debug, Clone)]
pub enum Hittable {
    Wall(Shield),
    Enemy(Player),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ServerMessage{
    pub sender_id: String,
    pub player: Player
}