use macroquad::prelude::*;
use serde_json::from_str;
use std::sync::Mutex;
use std::{io, process::exit, usize};

mod preferences;
use preferences::*;

mod models;
use models::*;

mod utils;
use utils::*;

use std::net::UdpSocket;
use std::sync::Arc;

use std::thread;

fn conf() -> Conf {
    Conf {
        window_title: String::from("FPS-CLIENT"),
        window_width: SCREEN_WIDTH as i32,
        window_height: SCREEN_HEIGHT as i32,
        fullscreen: false,
        ..Default::default()
    }
}
#[macroquad::main(conf)]
async fn main() {
    //send request from client:  echo -n 'Hello from client' | nc -u 127.0.0.1 4000
    let mut player = Player::new();
    player.current_map = String::from("map_one");

    let mut status = Status::EnterIP;
    //request_new_screen_size(SCREEN_WIDTH as f32, SCREEN_HEIGHT as f32);
    let mut server_addr = String::new();
    //init game engine
    let mut game_params = init_game_params();

    player.mini_map = game_params.mini_map.clone();
    let player = Arc::new(Mutex::new(player));

    set_cursor_grab(true);
    show_mouse(false);
    let enemies: Arc<Mutex<Option<Vec<Player>>>> = Arc::new(Mutex::new(None));
    // test
    // let mut enemy = Player::new();
    // enemy.name = "Sam".to_string();
    // enemy.position = Position::build(7.0, 1.0);
    // let enem = Some(vec![enemy]);
    // enemies = Arc::new(Mutex::new(enem));
    //end test

    let socket = Arc::new(UdpSocket::bind("0.0.0.0:0").unwrap());
    start_server_listener(
        Arc::clone(&socket),
        Arc::clone(&enemies),
        Arc::clone(&player),
        Arc::clone(&game_params.hittables),
    );

    loop {
        match status {
            Status::EnterIP => handle_ip_input(&mut status, &mut server_addr),
            Status::EnterName => {
                handle_name_input(&mut status, player.clone(), &server_addr);
            }
            Status::Run => handle_game_run(
                &server_addr,
                player.clone(),
                &mut game_params,
                &socket,
                Arc::clone(&enemies),
            ),
        }
        next_frame().await;
    }
}
fn init_game_params() -> GameParams {
    let wall_texture = Texture2D::from_file_with_format(include_bytes!("../assets/grey.png"), None);
    let sky_texture = Texture2D::from_file_with_format(include_bytes!("../assets/sky.png"), None);
    let arrow_texture =
        Texture2D::from_file_with_format(include_bytes!("../assets/small_arrow.png"), None);
    let eye_texture =
        Image::from_file_with_format(include_bytes!("../assets/eye_texture.png"), None).unwrap();

    let mini_map = match parse_map("assets/map_one.txt") {
        Ok(map) => map,
        Err(error) => {
            println!("Problem opening the file: {error:?}");
            exit(1);
        }
    };
    let mini_map_config = MiniMapConfig::new(
        &mini_map,
        MAP_WIDTH,
        MAP_HEIGHT,
        MAP_MARGIN_LEFT,
        MAP_MARGIN_TOP,
        BLACK,
    );
    let render_target = render_target_ex(
        MAIN_WIDTH,
        MAIN_HEIGHT,
        RenderTargetParams {
            sample_count: 1,
            depth: true,
        },
    );
    let world_up = vec3(0.0, 1.0, 0.0);
    let yaw: f32 = 1.18; //rotation around y axes
    let pitch: f32 = 0.0; //tilt up/down
    let front = vec3(
        yaw.cos() * pitch.cos(),
        pitch.sin(),
        yaw.sin() * pitch.cos(),
    )
    .normalize();
    let right = front.cross(world_up).normalize();
    let position = generate_position(&mini_map);
    let last_mouse_position: Vec2 = mouse_position().into();

    let shots = vec![];

    let hittables = Arc::new(Mutex::new(vec![]));
    add_shields(Arc::clone(&hittables), &mini_map);

    // let mut enemy = Player::new();
    // enemy.name = "Sam".to_string();
    // enemy.position = Position::build(7.0, 1.0);
    // hittables.push(Hittable::Enemy(enemy));

    GameParams {
        wall_texture,
        sky_texture,
        arrow_texture,
        eye_texture,
        mini_map_config,
        render_target,
        yaw,
        pitch,
        front,
        right,
        position,
        last_mouse_position,
        mini_map,
        world_up,
        shots,
        hittables,
    }
}
fn parse_map(file_path: &str) -> Result<Vec<Vec<bool>>, io::Error> {
    let content = read_file(file_path)?;
    if !is_map_valid(&content) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid Map Format",
        ));
    }
    Ok(map_to_slice(&content))
}
fn render_mini_map(mini_map: &Vec<Vec<bool>>, mini_map_config: &MiniMapConfig) {
    let mut horizontal_offset: f32 = mini_map_config.horizontal_offset as f32;
    let mut vertical_offset: f32 = mini_map_config.vertical_offset as f32;
    for line in mini_map {
        for cell in line {
            if *cell {
                draw_rectangle(
                    horizontal_offset,
                    vertical_offset,
                    mini_map_config.cell_width,
                    mini_map_config.cell_height,
                    mini_map_config.cell_color,
                );
            }
            horizontal_offset += mini_map_config.cell_width
        }
        horizontal_offset = mini_map_config.horizontal_offset as f32;
        vertical_offset += mini_map_config.cell_height;
    }
}
fn generate_position(map: &Vec<Vec<bool>>) -> Vec3 {
    let mut spaces: Vec<(usize, usize)> = vec![];
    for (z, line) in map.iter().enumerate() {
        for (x, cell) in line.iter().enumerate() {
            if !*cell {
                spaces.push((x, z));
            }
        }
    }
    let rand_index = generate_up_to(spaces.len());
    let x = spaces[rand_index].0 as f32;
    let z = spaces[rand_index].1 as f32;
    vec3(x, PLAYER_HEIGHT, z)
    //vec3(1.0, PLAYER_HEIGHT, 1.0)
}
fn draw_player_on_mini_map(
    player: &Player,
    mini_map: &Vec<Vec<bool>>,
    config: &MiniMapConfig,
    up_texture: &Texture2D,
) {
    let image_size = f32::min(config.cell_width, config.cell_height);

    //current position
    let index_x = f32::floor(player.position.x + 0.5);
    let index_z = f32::floor(player.position.z + 0.5);

    let mut x = config.horizontal_offset as f32 + player.position.x * config.cell_width;
    let mut z = config.vertical_offset as f32 + player.position.z * config.cell_height;

    //horizontal tunnel + horizontal pockets
    if mini_map[index_z as usize + 1][index_x as usize]
        && mini_map[index_z as usize - 1][index_x as usize]
    {
        z = config.vertical_offset as f32 + index_z * config.cell_height;
        //pocket on the right
        if mini_map[index_z as usize][index_x as usize + 1] {
            let max_x = config.horizontal_offset as f32 + index_x * config.cell_width;
            if x > max_x {
                x = max_x
            }
        }
        //pocket on the left
        if mini_map[index_z as usize][index_x as usize - 1] {
            let min_x = config.horizontal_offset as f32 + index_x * config.cell_width;
            if x < min_x {
                x = min_x
            }
        }
    }

    //vertical tunnel + vertical pocket
    if mini_map[index_z as usize][index_x as usize + 1]
        && mini_map[index_z as usize][index_x as usize - 1]
    {
        x = config.horizontal_offset as f32 + index_x * config.cell_width;
        //pocket up
        if mini_map[index_z as usize - 1][index_x as usize] {
            let min_z = config.vertical_offset as f32 + index_z * config.cell_height;
            if z < min_z {
                z = min_z;
            }
        }
        //pocket down
        if mini_map[index_z as usize + 1][index_x as usize] {
            let max_z = config.vertical_offset as f32 + index_z * config.cell_height;
            if z > max_z {
                z = max_z;
            }
        }
    }

    /*
    top left corner

      ____
     | ↗
     |

    */
    if mini_map[index_z as usize][index_x as usize - 1]
        && mini_map[index_z as usize - 1][index_x as usize]
        && !mini_map[index_z as usize][index_x as usize + 1]
        && !mini_map[index_z as usize + 1][index_x as usize]
    {
        let min_z = config.vertical_offset as f32 + index_z * config.cell_height;
        let min_x = config.horizontal_offset as f32 + index_x * config.cell_width;
        if x < min_x {
            x = min_x;
        }
        if z < min_z {
            z = min_z;
        }
    }

    /*
    top right corner
    ____
       ↗|
        |

    */

    if mini_map[index_z as usize - 1][index_x as usize]
        && mini_map[index_z as usize][index_x as usize + 1]
        && !mini_map[index_z as usize][index_x as usize - 1]
        && !mini_map[index_z as usize + 1][index_x as usize]
    {
        let max_x = config.horizontal_offset as f32 + index_x * config.cell_width;
        let min_z = config.vertical_offset as f32 + index_z * config.cell_height;
        if x > max_x {
            x = max_x;
        }
        if z < min_z {
            z = min_z;
        }
    }

    /*
        bottom right corner

            |
           ↗|
        ----

    */
    if mini_map[index_z as usize][index_x as usize + 1]
        && mini_map[index_z as usize + 1][index_x as usize]
        && !mini_map[index_z as usize - 1][index_x as usize]
        && !mini_map[index_z as usize][index_x as usize - 1]
    {
        let max_x = config.horizontal_offset as f32 + index_x * config.cell_width;
        let max_z = config.vertical_offset as f32 + index_z * config.cell_height;
        if x > max_x {
            x = max_x
        }
        if z > max_z {
            z = max_z
        }
    }

    /*
       bottom left corner

       |
       |↗
       -----

    */

    if mini_map[index_z as usize][index_x as usize - 1]
        && mini_map[index_z as usize + 1][index_x as usize]
        && !mini_map[index_z as usize - 1][index_x as usize]
        && !mini_map[index_z as usize][index_x as usize + 1]
    {
        let min_x = config.horizontal_offset as f32 + index_x * config.cell_width;
        let max_z = config.vertical_offset as f32 + index_z * config.cell_height;
        if x < min_x {
            x = min_x
        }
        if z > max_z {
            z = max_z
        }
    }

    /*
    t-junction

    __________
    ___  ↗ ___
       |  |
       |  |

    */
    if mini_map[index_z as usize - 1][index_x as usize]
        && !mini_map[index_z as usize][index_x as usize - 1]
        && !mini_map[index_z as usize][index_x as usize + 1]
        && !mini_map[index_z as usize + 1][index_x as usize]
    {
        let min_z = config.vertical_offset as f32 + index_z * config.cell_height;
        if z < min_z {
            z = min_z;
        }
    }

    /*
     right t-junction

    |  |__
    | ↗ __
    |  |

    */
    if mini_map[index_z as usize][index_x as usize - 1]
        && !mini_map[index_z as usize + 1][index_x as usize]
        && !mini_map[index_z as usize - 1][index_x as usize]
        && !mini_map[index_z as usize][index_x as usize + 1]
    {
        let min_x = config.horizontal_offset as f32 + index_x * config.cell_width;
        if x < min_x {
            x = min_x;
        }
    }

    /*
        left t-junction

         ____|  |
         ____  ↗|
             |  |

    */
    if mini_map[index_z as usize][index_x as usize + 1]
        && !mini_map[index_z as usize - 1][index_x as usize]
        && !mini_map[index_z as usize + 1][index_x as usize]
        && !mini_map[index_z as usize][index_x as usize - 1]
    {
        let max_x = config.horizontal_offset as f32 + index_x * config.cell_width;
        if x > max_x {
            x = max_x;
        }
    }

    /*
       up t-junction

         ___|  |___
              ↗
         -----------

    */

    if mini_map[index_z as usize + 1][index_x as usize]
        && !mini_map[index_z as usize][index_x as usize - 1]
        && !mini_map[index_z as usize][index_x as usize + 1]
        && !mini_map[index_z as usize - 1][index_x as usize]
    {
        let max_z = config.vertical_offset as f32 + index_z * config.cell_height;
        if z > max_z {
            z = max_z;
        }
    }

    /*
        right pocket
        ___
          ↗|
        ---
    */

    let size = vec2(image_size, image_size);
    draw_texture_ex(
        up_texture,
        x,
        z,
        WHITE,
        DrawTextureParams {
            dest_size: Some(size),
            source: None,
            rotation: player.orientation,
            flip_x: false,
            flip_y: false,
            pivot: None,
        },
    );
}
fn draw_enemy_on_minimap(
    player: &Player,
    mini_map: &Vec<Vec<bool>>,
    config: &MiniMapConfig,
    color: Color,
) {
    let image_size = f32::min(config.cell_width, config.cell_height);

    //current position
    let index_x = f32::floor(player.position.x + 0.5);
    let index_z = f32::floor(player.position.z + 0.5);

    let mut x = config.horizontal_offset as f32 + player.position.x * config.cell_width;
    let mut z = config.vertical_offset as f32 + player.position.z * config.cell_height;

    //horizontal tunnel + horizontal pockets
    if mini_map[index_z as usize + 1][index_x as usize]
        && mini_map[index_z as usize - 1][index_x as usize]
    {
        z = config.vertical_offset as f32 + index_z * config.cell_height;
        //pocket on the right
        if mini_map[index_z as usize][index_x as usize + 1] {
            let max_x = config.horizontal_offset as f32 + index_x * config.cell_width;
            if x > max_x {
                x = max_x
            }
        }
        //pocket on the left
        if mini_map[index_z as usize][index_x as usize - 1] {
            let min_x = config.horizontal_offset as f32 + index_x * config.cell_width;
            if x < min_x {
                x = min_x
            }
        }
    }

    //vertical tunnel + vertical pocket
    if mini_map[index_z as usize][index_x as usize + 1]
        && mini_map[index_z as usize][index_x as usize - 1]
    {
        x = config.horizontal_offset as f32 + index_x * config.cell_width;
        //pocket up
        if mini_map[index_z as usize - 1][index_x as usize] {
            let min_z = config.vertical_offset as f32 + index_z * config.cell_height;
            if z < min_z {
                z = min_z;
            }
        }
        //pocket down
        if mini_map[index_z as usize + 1][index_x as usize] {
            let max_z = config.vertical_offset as f32 + index_z * config.cell_height;
            if z > max_z {
                z = max_z;
            }
        }
    }

    /*
    top left corner

      ____
     | ↗
     |

    */
    if mini_map[index_z as usize][index_x as usize - 1]
        && mini_map[index_z as usize - 1][index_x as usize]
        && !mini_map[index_z as usize][index_x as usize + 1]
        && !mini_map[index_z as usize + 1][index_x as usize]
    {
        let min_z = config.vertical_offset as f32 + index_z * config.cell_height;
        let min_x = config.horizontal_offset as f32 + index_x * config.cell_width;
        if x < min_x {
            x = min_x;
        }
        if z < min_z {
            z = min_z;
        }
    }

    /*
    top right corner
    ____
       ↗|
        |

    */

    if mini_map[index_z as usize - 1][index_x as usize]
        && mini_map[index_z as usize][index_x as usize + 1]
        && !mini_map[index_z as usize][index_x as usize - 1]
        && !mini_map[index_z as usize + 1][index_x as usize]
    {
        let max_x = config.horizontal_offset as f32 + index_x * config.cell_width;
        let min_z = config.vertical_offset as f32 + index_z * config.cell_height;
        if x > max_x {
            x = max_x;
        }
        if z < min_z {
            z = min_z;
        }
    }

    /*
        bottom right corner

            |
           ↗|
        ----

    */
    if mini_map[index_z as usize][index_x as usize + 1]
        && mini_map[index_z as usize + 1][index_x as usize]
        && !mini_map[index_z as usize - 1][index_x as usize]
        && !mini_map[index_z as usize][index_x as usize - 1]
    {
        let max_x = config.horizontal_offset as f32 + index_x * config.cell_width;
        let max_z = config.vertical_offset as f32 + index_z * config.cell_height;
        if x > max_x {
            x = max_x
        }
        if z > max_z {
            z = max_z
        }
    }

    /*
       bottom left corner

       |
       |↗
       -----

    */

    if mini_map[index_z as usize][index_x as usize - 1]
        && mini_map[index_z as usize + 1][index_x as usize]
        && !mini_map[index_z as usize - 1][index_x as usize]
        && !mini_map[index_z as usize][index_x as usize + 1]
    {
        let min_x = config.horizontal_offset as f32 + index_x * config.cell_width;
        let max_z = config.vertical_offset as f32 + index_z * config.cell_height;
        if x < min_x {
            x = min_x
        }
        if z > max_z {
            z = max_z
        }
    }

    /*
    t-junction

    __________
    ___  ↗ ___
       |  |
       |  |

    */
    if mini_map[index_z as usize - 1][index_x as usize]
        && !mini_map[index_z as usize][index_x as usize - 1]
        && !mini_map[index_z as usize][index_x as usize + 1]
        && !mini_map[index_z as usize + 1][index_x as usize]
    {
        let min_z = config.vertical_offset as f32 + index_z * config.cell_height;
        if z < min_z {
            z = min_z;
        }
    }

    /*
     right t-junction

    |  |__
    | ↗ __
    |  |

    */
    if mini_map[index_z as usize][index_x as usize - 1]
        && !mini_map[index_z as usize + 1][index_x as usize]
        && !mini_map[index_z as usize - 1][index_x as usize]
        && !mini_map[index_z as usize][index_x as usize + 1]
    {
        let min_x = config.horizontal_offset as f32 + index_x * config.cell_width;
        if x < min_x {
            x = min_x;
        }
    }

    /*
        left t-junction

         ____|  |
         ____  ↗|
             |  |

    */
    if mini_map[index_z as usize][index_x as usize + 1]
        && !mini_map[index_z as usize - 1][index_x as usize]
        && !mini_map[index_z as usize + 1][index_x as usize]
        && !mini_map[index_z as usize][index_x as usize - 1]
    {
        let max_x = config.horizontal_offset as f32 + index_x * config.cell_width;
        if x > max_x {
            x = max_x;
        }
    }

    /*
       up t-junction

         ___|  |___
              ↗
         -----------

    */

    if mini_map[index_z as usize + 1][index_x as usize]
        && !mini_map[index_z as usize][index_x as usize - 1]
        && !mini_map[index_z as usize][index_x as usize + 1]
        && !mini_map[index_z as usize - 1][index_x as usize]
    {
        let max_z = config.vertical_offset as f32 + index_z * config.cell_height;
        if z > max_z {
            z = max_z;
        }
    }

    /*
        right pocket
        ___
          ↗|
        ---
    */
    draw_circle(
        x + image_size / 2.0,
        z + image_size / 2.0,
        image_size / 2.5,
        color,
    );
}
fn draw_walls(mini_map: &Vec<Vec<bool>>, texture: Option<&Texture2D>, color: Color) {
    for (z, line) in mini_map.into_iter().enumerate() {
        for (x, cell) in line.iter().enumerate() {
            if *cell {
                let position = vec3(x as f32, 1.0, z as f32);
                let size = vec3(1.0, 1.0, 1.0);
                draw_cube(position, size, texture, color);
            }
        }
    }
}
fn handle_wall_collisions(
    mini_map: &Vec<Vec<bool>>,
    prev_pos: Vec3,
    position: &mut Vec3,
    gap: f32,
) {
    let mut pos = position.clone();
    pos.z = prev_pos.z;
    let points = [
        (pos.x + 0.5 + gap, pos.z + 0.5 + gap),
        (pos.x + 0.5 - gap, pos.z + 0.5 - gap),
        (pos.x + 0.5 + gap, pos.z + 0.5 - gap),
        (pos.x + 0.5 - gap, pos.z + 0.5 + gap),
    ];
    let floors = points.map(|item| (f32::floor(item.0), f32::floor(item.1)));
    for floor in floors {
        if mini_map[floor.1 as usize][floor.0 as usize] {
            position.x = prev_pos.x;
            break;
        }
    }

    let mut pos = position.clone();
    pos.x = prev_pos.x;

    let points = [
        (pos.x + 0.5 + gap, pos.z + 0.5 + gap),
        (pos.x + 0.5 - gap, pos.z + 0.5 - gap),
        (pos.x + 0.5 + gap, pos.z + 0.5 - gap),
        (pos.x + 0.5 - gap, pos.z + 0.5 + gap),
    ];
    let floors = points.map(|item| (f32::floor(item.0), f32::floor(item.1)));
    for floor in floors {
        if mini_map[floor.1 as usize][floor.0 as usize] {
            position.z = prev_pos.z;
            break;
        }
    }
}
//handlers
fn handle_ip_input(status: &mut Status, server_addr: &mut String) {
    clear_background(BLACK);
    let mut server_addr_display =
        "Enter server IP addrsss. Example: 127.0.0.1:4000    ".to_string();
    server_addr_display.push_str(server_addr);
    draw_text(server_addr_display.as_str(), 10.0, 20.0, 20.0, LIGHTGRAY);

    if let Some(c) = get_char_pressed() {
        if c == 3 as char || c == 13 as char {
            *status = Status::EnterName;
            return;
        }

        if is_valid_ip_char(c) {
            server_addr.push(c);
        }
    }

    if is_key_pressed(KeyCode::Backspace) {
        server_addr.pop();
    }
    if is_key_pressed(KeyCode::Escape) {
        exit(0);
    }
}
fn handle_name_input(status: &mut Status, player_ref: Arc<Mutex<Player>>, server_addr: &String) {
    clear_background(BLACK);

    match player_ref.lock() {
        Ok(mut player) => {
            let mut server_addr_display =
                "Enter server IP addrsss. Example: 127.0.0.1:4000    ".to_string();
            server_addr_display.push_str(server_addr);

            let mut player_name_display = "Enter your name:     ".to_string();
            player_name_display.push_str(&player.name);

            draw_text(server_addr_display.as_str(), 10.0, 20.0, 20.0, LIGHTGRAY);
            draw_text(player_name_display.as_str(), 10.0, 40.0, 20.0, LIGHTGRAY);

            if let Some(c) = get_char_pressed() {
                if c == 3 as char || c == 13 as char {
                    if player.name.len() > 2 {
                        *status = Status::Run;
                        return;
                    }
                }
                if player.name.len() < MAX_NAME_LENGTH && is_valid_name_char(c) {
                    player.name.push(c);
                }
            }

            if is_key_pressed(KeyCode::Backspace) {
                player.name.pop();
            }
        }
        Err(e) => println!("Error while locking player: {:?}", e),
    }

    if is_key_pressed(KeyCode::Escape) {
        exit(0);
    }
}
fn handle_game_run(
    server_addr: &String,
    player_ref: Arc<Mutex<Player>>,
    game_params: &mut GameParams,
    socket: &Arc<UdpSocket>,
    enemies: Arc<Mutex<Option<Vec<Player>>>>,
) {
    let delta = get_frame_time();
    let prev_pos = game_params.position.clone();

    match player_ref.lock() {
        Ok(mut player) => {
            let _player = &player.clone(); //to use in json parse
            if is_key_pressed(KeyCode::Escape) {
                //player.is_active = false;
                player.player_status = PlayerStatus::Disconnent;
                if let Ok(message_to_server) = serde_json::to_string(_player) {
                    let _ = socket.send_to(message_to_server.as_bytes(), server_addr);
                }
                exit(0);
            }
            if is_key_down(KeyCode::Up) || is_key_down(KeyCode::W) {
                game_params.position += game_params.front * MOVE_SPEED;
            }
            if is_key_down(KeyCode::Down) || is_key_down(KeyCode::S) {
                game_params.position -= game_params.front * MOVE_SPEED;
            }
            if is_key_down(KeyCode::Left) || is_key_down(KeyCode::A) {
                game_params.position -= game_params.right * MOVE_SPEED;
            }
            if is_key_down(KeyCode::Right) || is_key_down(KeyCode::D) {
                game_params.position += game_params.right * MOVE_SPEED;
            }

            let gap: f32 = 0.05;
            handle_wall_collisions(
                &game_params.mini_map,
                prev_pos,
                &mut game_params.position,
                gap,
            );
            let mouse_position: Vec2 = mouse_position().into();
            let mouse_delta = mouse_position - game_params.last_mouse_position;
            game_params.last_mouse_position = mouse_position;
            game_params.yaw += mouse_delta.x * delta * LOOK_SPEED;
            game_params.pitch += mouse_delta.y * delta * -LOOK_SPEED;
            game_params.pitch = if game_params.pitch > MAX_PITCH {
                MAX_PITCH
            } else {
                game_params.pitch
            };
            game_params.pitch = if game_params.pitch < MIN_PITCH {
                MIN_PITCH
            } else {
                game_params.pitch
            };
            game_params.front = vec3(
                game_params.yaw.cos() * game_params.pitch.cos(),
                game_params.pitch.sin(),
                game_params.yaw.sin() * game_params.pitch.cos(),
            )
            .normalize();
            game_params.right = game_params.front.cross(game_params.world_up).normalize();
            let up = game_params.right.cross(game_params.front).normalize();
            game_params.position.y = PLAYER_HEIGHT;
            //2d
            set_default_camera();
            clear_background(WHITE);
            player.position = Position::build(game_params.position.x, game_params.position.z);
            //find projection of front on x_z plane
            let p = game_params.front.dot(game_params.world_up) * game_params.world_up;
            let orientation = (game_params.front - p).normalize();
            player.orientation =
                orientaion_to_degrees(vec3(orientation.x, orientation.y, orientation.z));
            draw_rectangle_lines(
                MAP_MARGIN_LEFT as f32,
                MAP_MARGIN_TOP as f32,
                MAP_WIDTH as f32,
                MAP_HEIGHT as f32,
                2.0,
                DARKGRAY,
            );
            draw_text(
                &player.name,
                NAME_MARGIN_LEFT as f32,
                NAME_MARGIN_TOP as f32,
                20.0,
                DARKGRAY,
            );
            draw_text(
                format!("{}", player.score).as_str(),
                SCORE_MARGIN_LEFT as f32,
                NAME_MARGIN_TOP as f32,
                20.0,
                DARKGRAY,
            );
            render_mini_map(&game_params.mini_map, &game_params.mini_map_config);
            draw_player_on_mini_map(
                &player,
                &game_params.mini_map,
                &game_params.mini_map_config,
                &game_params.arrow_texture,
            );
            draw_texture_ex(
                &game_params.render_target.texture,
                MAIN_MARGIN_LEFT as f32,
                (MAIN_MARGIN_TOP + MAIN_HEIGHT) as f32,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(Vec2::new(MAIN_WIDTH as f32, -1.0 * MAIN_HEIGHT as f32)),
                    ..Default::default()
                },
            );

            //enemies
            if let Ok(enemies_result) = enemies.lock() {
                if let Some(enemies) = enemies_result.clone() {
                    draw_enemy_names_and_scores(&enemies);
                    draw_enemies_on_minimap(&enemies, &game_params);
                }
            }

            //3d
            set_camera(&Camera3D {
                render_target: Some(game_params.render_target.clone()),
                position: game_params.position,
                up: up,
                target: game_params.position + game_params.front,
                ..Default::default()
            });
            clear_background(LIGHTGRAY);
            draw_grid(50, 1., BLACK, GRAY);
            draw_walls(
                &game_params.mini_map,
                Some(&game_params.wall_texture),
                WHITE,
            );
            //sky
            let center = vec3(-20.0, 5.0, -20.0);
            let size = vec2(100.0, 100.0);
            draw_plane(center, size, Some(&game_params.sky_texture), WHITE);
            //ground
            let center = vec3(-50.0, -0.1, -50.0);
            let size = vec2(100.0, 100.0);
            draw_plane(center, size, None, BROWN);

            //enemies
            if let Ok(enemies_result) = enemies.lock() {
                if let Some(enemies) = enemies_result.clone() {
                    for enemy in enemies {
                        let bytes = game_params.eye_texture.bytes.clone();
                        let width = game_params.eye_texture.width as u32;
                        let height = game_params.eye_texture.height;

                        let a = enemy.orientation.to_degrees() as u32;
                        let index = a * width * 4;
                        let mut top_half = bytes[..index as usize].to_vec();
                        let mut bottom_half = bytes[index as usize..].to_vec();
                        let mut bytes = vec![];
                        bytes.append(&mut bottom_half);
                        bytes.append(&mut top_half);
                        let image = Image {
                            bytes,
                            width: width as u16,
                            height,
                        };
                        let texture = Texture2D::from_image(&image);
                        draw_sphere(
                            vec3(enemy.position.x, PLAYER_HEIGHT, enemy.position.z),
                            ENEMY_RADIUS,
                            Some(&texture),
                            WHITE,
                        );
                    }
                }
            }

            //shooting
            if is_mouse_button_pressed(MouseButton::Left) {
                match game_params.hittables.lock() {
                    Ok(hittables) => {
                        let mut closest_hit_option: Option<Hit> = None;

                        let start = vec3(player.position.x, 0.95, player.position.z)
                            + game_params.front / 10.0;

                        for hittable in hittables.iter() {
                            if let Hittable::Wall(shield) = hittable {
                                let hit_option = shield.hit(start, game_params.front);
                                if let Some(hit) = hit_option {
                                    if let Some(ref closest_hit) = closest_hit_option {
                                        if hit.t < closest_hit.t {
                                            closest_hit_option = Some(hit);
                                        }
                                    } else {
                                        closest_hit_option = Some(hit);
                                    }
                                };
                            }
                            //implement for enemy
                            if let Hittable::Enemy(player) = hittable {
                                let hit_option = player.hit(start, game_params.front);
                                if let Some(hit) = hit_option {
                                    if let Some(ref closest_hit) = closest_hit_option {
                                        if hit.t < closest_hit.t {
                                            closest_hit_option = Some(hit);
                                        }
                                    } else {
                                        closest_hit_option = Some(hit);
                                    }
                                }
                            }
                        }

                        let end = if let Some(closest_hit) = closest_hit_option {
                            match closest_hit.hittable {
                                Hittable::Wall(_) => println!("hit wall: {:?}", closest_hit.p),
                                Hittable::Enemy(mut player) => {
                                    //hit enemy
                                    println!("hit enemy: {:?}", player.name);
                                    //update score
                                    match player_ref.lock() {
                                        Ok(mut player) => player.score += 1,
                                        Err(e) => println!("Error while locking player {:?}", e),
                                    }
                                    //remove from hitables
                                    match game_params.hittables.lock() {
                                        Ok(mut hittables) => {
                                            *hittables = hittables
                                                .iter()
                                                .filter(|hittable| {
                                                    if let Hittable::Enemy(enemy) = hittable {
                                                        enemy.id != player.id
                                                    } else {
                                                        true
                                                    }
                                                })
                                                .cloned()
                                                .collect();
                                        }
                                        Err(e) => println!("Error while locking hittables {:?}", e),
                                    }
                                    //notify server
                                    player.player_status = PlayerStatus::Killed;
                                    if let Ok(message_to_server) = serde_json::to_string(&player) {
                                        send_message_to_server(
                                            socket,
                                            server_addr,
                                            &message_to_server,
                                        );
                                    }
                                }
                            }
                            closest_hit.p
                        } else {
                            println!("miss");
                            start + game_params.front * MAX_SHOT_RANGE
                        };

                        let shot = Shot {
                            start,
                            end,
                            time_out: SHOT_DURATION,
                            color: RED,
                        };
                        game_params.shots.push(shot);
                    }
                    Err(e) => println!("Error while locking hittables {:?}", e),
                }
            }

            draw_shots(&game_params.shots);
            remove_shots(&mut game_params.shots);

            //draw hittables
            /*
            for hittable in &game_params.hittables {
                if let Hittable::Wall(shield) = hittable {
                    draw_sphere(shield.q, 0.05, None, PURPLE);
                    draw_sphere(shield.q + shield.u, 0.05, None, GREEN);
                    draw_sphere(shield.q + shield.v, 0.05, None, BLUE);
                }
            }
            */

            if let Ok(message_to_server) = serde_json::to_string(_player) {
                send_message_to_server(socket, server_addr, &message_to_server);
            }
        }
        Err(e) => {
            println!("Error while locking player: {:?}", e)
        }
    }
}
fn start_server_listener(
    socket: Arc<UdpSocket>,
    enemies: Arc<Mutex<Option<Vec<Player>>>>,
    player: Arc<Mutex<Player>>,
    hittables: Arc<Mutex<Vec<Hittable>>>,
) {
    let player_id = player.lock().unwrap().id.clone();
    //Server response listener
    thread::spawn(move || loop {
        let mut buffer = [0u8; 1024];
        if let Ok((size, _)) = socket.recv_from(&mut buffer) {
            // println!(
            //     "Received {} bytes from {}: {}",
            //     size,
            //     src,
            //     std::str::from_utf8(&buffer[..size]).unwrap_or("<invalid UTF-8>")
            // );

            if let Ok(players_str) = std::str::from_utf8(&buffer[..size]) {
                //let enemies_result = from_str::<Vec<Player>>(enemies_str);
                match from_str::<Vec<Player>>(players_str) {
                    Ok(players) => {
                        //clear hittables from enemies
                        match hittables.lock() {
                            Ok(mut hittables_locked) => {
                                *hittables_locked = hittables_locked
                                    .iter()
                                    .filter(|item| {
                                        if let Hittable::Enemy(_) = item {
                                            false
                                        } else {
                                            true
                                        }
                                    })
                                    .cloned()
                                    .collect();
                            }
                            Err(e) => println!("Error while locking hittables {:?}", e),
                        }
                        //filter player and handle if killed
                        let mut enemies_local_option: Option<Vec<Player>> = None;
                        for _player in players {
                            if _player.id == player_id {
                                if let PlayerStatus::Killed = _player.player_status {
                                    //player is killed. update position and status
                                    match player.lock() {
                                        Ok(mut player_locked) => {
                                            println!("Player {} killed", player_locked.name);
                                            let position =
                                                generate_position(&player_locked.mini_map);
                                            player_locked.position =
                                                Position::build(position.x, position.z);
                                            player_locked.player_status = PlayerStatus::Active;
                                        }
                                        Err(e) => println!("Error while locking player: {:?}", e),
                                    }
                                }
                            } else {
                                //collect enemies
                                if let Some(ref mut enemies_local) = enemies_local_option {
                                    enemies_local.push(_player.clone());
                                } else {
                                    enemies_local_option = Some(vec![_player.clone()]);
                                }

                                //update hittables
                                match hittables.lock() {
                                    Ok(mut hittables_locked) => {
                                        hittables_locked.push(Hittable::Enemy(_player));
                                    }
                                    Err(e) => println!("Error while locking hittables {:?}", e),
                                }
                            }
                        }
                        //update enemies
                        match enemies.lock() {
                            Ok(mut enemies_locked) => *enemies_locked = enemies_local_option,
                            Err(e) => println!("Error while locking enemies: {:?}", e),
                        }
                    }
                    Err(e) => println!("Error while parsing players: {e}"),
                }
            } else {
                println!("no enemies...",);
                // let enemies_locked_result = enemies.lock();
                // match enemies_locked_result {
                //     Ok(mut enemies_locked) => *enemies_locked = None,
                //     Err(e) => println!("Error while locking: {e}"),
                // }
                match enemies.lock() {
                    Ok(mut enemies_locked) => *enemies_locked = None,
                    Err(e) => println!("Error while locking emenies: {e}"),
                }
                //clear hittables from enemies
                match hittables.lock() {
                    Ok(mut hittables_locked) => {
                        *hittables_locked = hittables_locked
                            .iter()
                            .filter(|item| {
                                if let Hittable::Enemy(_) = item {
                                    false
                                } else {
                                    true
                                }
                            })
                            .cloned()
                            .collect();
                    }
                    Err(e) => println!("Error while locking hittables {:?}", e),
                }
            }
        }
    });
}
fn draw_enemy_names_and_scores(enemies: &Vec<Player>) {
    let mut top_offset = NAME_MARGIN_TOP as f32 + 35.0;
    for enemy in enemies {
        if let PlayerStatus::Active = enemy.player_status {
            draw_text(
                &enemy.name,
                NAME_MARGIN_LEFT as f32,
                top_offset,
                20.0,
                DARKGRAY,
            );
            draw_text(
                format!("{}", enemy.score).as_str(),
                SCORE_MARGIN_LEFT as f32,
                top_offset,
                20.0,
                DARKGRAY,
            );
            top_offset += 35.0;
        }
    }
}
fn draw_enemies_on_minimap(enemies: &Vec<Player>, game_params: &GameParams) {
    for enemy in enemies {
        if let PlayerStatus::Active = enemy.player_status {
            draw_enemy_on_minimap(
                &enemy,
                &game_params.mini_map,
                &game_params.mini_map_config,
                RED,
            );
        }
    }
}
fn draw_shots(shots: &Vec<Shot>) {
    for shot in shots {
        draw_line_3d(shot.start, shot.end, shot.color);
    }
}
fn remove_shots(shots: &mut Vec<Shot>) {
    for i in 0..shots.len() {
        shots[i].time_out -= 1;
    }
    let filtered: Vec<Shot> = shots
        .iter()
        .filter(|shot| shot.time_out > 0)
        .cloned()
        .collect();
    *shots = filtered;
}
fn add_shields(hittables_ref: Arc<Mutex<Vec<Hittable>>>, mini_map: &Vec<Vec<bool>>) {
    match hittables_ref.lock() {
        Ok(mut hittables) => {
            ////////////////

            for (z, row) in mini_map.iter().enumerate() {
                for (x, cell) in row.iter().enumerate() {
                    if !cell {
                        //check cell up
                        if mini_map[z - 1][x] {
                            let shield = Shield::new(
                                vec3(x as f32 - 0.5, 0.5, z as f32 - 0.5),
                                vec3(1.0, 0.0, 0.0),
                                vec3(0.0, 1.0, 0.0),
                            );
                            hittables.push(Hittable::Wall(shield));
                        }
                        //check cell right
                        if mini_map[z][x + 1] {
                            let shield = Shield::new(
                                vec3(x as f32 + 0.5, 0.5, z as f32 - 0.5),
                                vec3(0.0, 0.0, 1.0),
                                vec3(0.0, 1.0, 0.0),
                            );
                            hittables.push(Hittable::Wall(shield));
                        }
                        //check cell bottom
                        if mini_map[z + 1][x] {
                            let shield = Shield::new(
                                vec3(x as f32 - 0.5, 0.5, z as f32 + 0.5),
                                vec3(1.0, 0.0, 0.0),
                                vec3(0.0, 1.0, 0.0),
                            );
                            hittables.push(Hittable::Wall(shield));
                        }
                        //check cell left
                        if mini_map[z][x - 1] {
                            let shield = Shield::new(
                                vec3(x as f32 - 0.5, 0.5, z as f32 - 0.5),
                                vec3(0.0, 0.0, 1.0),
                                vec3(0.0, 1.0, 0.0),
                            );
                            hittables.push(Hittable::Wall(shield));
                        }
                    }
                }
            }

            /////////////////
        }
        Err(e) => println!("Error while locking hittables {:?}", e),
    }

    //cell down from (1, 2)
    // let shield = Shield::new(
    //     vec3(1.5, 0.5, 1.5),
    //     vec3(1.0, 0.0, 0.0),
    //     vec3(0.0, 1.0, 0.0),
    // );
    // hittables.push(Hittable::Wall(shield));

    // //cell up from (1,1)
    // let shield = Shield::new(
    //     vec3(0.5, 0.5, 0.5),
    //     vec3(1.0, 0.0, 0.0),
    //     vec3(0.0, 1.0, 0.0),
    // );
    // hittables.push(Hittable::Wall(shield));

    // //cell up from (1,2)
    // let shield = Shield::new(
    //     vec3(1.5, 0.5, 0.5),
    //     vec3(1.0, 0.0, 0.0),
    //     vec3(0.0, 1.0, 0.0),
    // );
    // hittables.push(Hittable::Wall(shield));

    //9, 0, 1
    // let shield = Shield::new(
    //     vec3(9.5, 0.5, 0.5),
    //     vec3(0.0, 0.0, 1.0),
    //     vec3(0.0, 1.0, 0.0),
    // );
    // hittables.push(Hittable::Wall(shield));
}
fn send_message_to_server(socket: &Arc<UdpSocket>, server_addr: &String, message_to_server: &str) {
    if let Err(e) = socket.send_to(message_to_server.as_bytes(), server_addr) {
        println!(
            "Error while sending message {} to server: {:?}",
            message_to_server, e
        );
    }
}
