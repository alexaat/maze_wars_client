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

use std::{fs, thread};

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

    let mut move_speed = MOVE_SPEED;
    let mut look_speed = LOOK_SPEED;

    get_settings(&mut move_speed, &mut look_speed);

    let mut status = Status::EnterIP;
    let mut server_addr = String::new();
    let mut player_name = String::new();
    let mut map_path = String::from(DEFAULT_MAP_PATH);

    let mut game_params: Option<GameParams> = None;
    let mut player: Option<Arc<Mutex<Player>>> = None;

    let font = load_ttf_font("fonts/AltoMono.ttf").await.unwrap();

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

    //update server on the first round of the loop
    let mut is_first_tun = true;

    let mut fps: f32 = 0.0;
    let mut frame_counter: u32 = 0;
    let mut prev_time = get_ms();

    let mut selexted_map_index = 0;

    loop {
        frame_counter += 1;
        if frame_counter > 60 {
            let current_time = get_ms();

            if let Some(prev) = prev_time {
                if let Some(current) = current_time {
                    let diff = current - prev;
                    let diff = diff as f32 / 1000.0;
                    fps = frame_counter as f32 / diff;
                    frame_counter = 0;
                    prev_time = Some(current);
                }
            }
        }
        match status {
            Status::EnterIP => handle_ip_input(&mut status, &mut server_addr),
            Status::EnterName => handle_name_input(&mut status, &mut player_name, &server_addr),
            Status::SelectMap => select_map_handler(
                &mut status,
                &mut map_path,
                &server_addr,
                &player_name,
                &mut selexted_map_index,
            ),
            Status::Init => init_game_handler(
                &mut status,
                &mut game_params,
                &mut player,
                &player_name,
                &map_path,
            ),
            Status::StartServerListener => {
                if let Some(ref mut _game_params) = game_params {
                    if let Some(ref _player) = player {
                        start_server_listener(
                            Arc::clone(&socket),
                            Arc::clone(&enemies),
                            _player.clone(),
                            Arc::clone(&_game_params.hittables),
                            server_addr.clone(),
                        );
                        status = Status::Run;
                    } else {
                        println!("error while initialisation player");
                        exit(0);
                    }
                } else {
                    println!("error while initialisation game parameters");
                    exit(0);
                }
            }
            Status::Run => {
                if let Some(ref mut _game_params) = game_params {
                    if let Some(ref _player) = player {
                        handle_game_run(
                            &server_addr,
                            Arc::clone(&_player),
                            _game_params,
                            &socket,
                            Arc::clone(&enemies),
                            &mut is_first_tun,
                            fps,
                            &font,
                            move_speed,
                            look_speed,
                        );
                    } else {
                        println!("error while initialisation player");
                        exit(0);
                    }
                } else {
                    println!("error while initialisation game parameters");
                    exit(0);
                }
            }
        }
        next_frame().await;
    }
}
fn init_player(game_params: &GameParams, player_name: &String, map_path: &String) -> Player {
    let mut player = Player::new();
    player.name = String::from(player_name);
    player.current_map = String::from(map_path);
    player.mini_map = game_params.mini_map.clone();

    let yaw: f32 = 0.0; //rotation around y axes
    let pitch: f32 = 0.0; //tilt up/down
    let front = vec3(
        yaw.cos() * pitch.cos(),
        pitch.sin(),
        yaw.sin() * pitch.cos(),
    )
    .normalize();
    let right = front.cross(game_params.world_up).normalize();
    let position_vec3 = generate_position(&game_params.mini_map);

    player.yaw = yaw;
    player.pitch = pitch;
    player.front = front;
    player.right = right;
    player.position_vec3 = position_vec3;
    player.position = Position {
        x: position_vec3.x,
        z: position_vec3.z,
    };
    player
}
fn init_game_params(map_path: &String) -> GameParams {
    let wall_texture = Texture2D::from_file_with_format(include_bytes!("../assets/bricks.png"), None);
    let sky_texture = Texture2D::from_file_with_format(include_bytes!("../assets/sky.png"), None);
    let arrow_texture =
        Texture2D::from_file_with_format(include_bytes!("../assets/small_arrow.png"), None);
    let eye_texture =
        Image::from_file_with_format(include_bytes!("../assets/eye_texture.png"), None).unwrap();
    let floor_texture = Texture2D::from_file_with_format(include_bytes!("../assets/patio448.png"), None);

    let mini_map = match parse_map(map_path) {
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
    let last_mouse_position: Vec2 = mouse_position().into();

    let shots = vec![];

    let hittables = Arc::new(Mutex::new(vec![]));
    add_shields(Arc::clone(&hittables), &mini_map);

    GameParams {
        wall_texture,
        sky_texture,
        arrow_texture,
        eye_texture,
        floor_texture,
        mini_map_config,
        render_target,
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
    draw_text(
        server_addr_display.as_str(),
        10.0,
        20.0,
        CONSOLE_FONT_SIZE,
        LIGHTGRAY,
    );

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
fn handle_name_input(status: &mut Status, player_name: &mut String, server_addr: &String) {
    clear_background(BLACK);

    let mut server_addr_display =
        "Enter server IP addrsss. Example: 127.0.0.1:4000    ".to_string();
    server_addr_display.push_str(server_addr);

    let mut player_name_display = "Enter your name:     ".to_string();
    player_name_display.push_str(&player_name);

    draw_text(
        server_addr_display.as_str(),
        10.0,
        20.0,
        CONSOLE_FONT_SIZE,
        LIGHTGRAY,
    );
    draw_text(
        player_name_display.as_str(),
        10.0,
        40.0,
        CONSOLE_FONT_SIZE,
        LIGHTGRAY,
    );

    if let Some(c) = get_char_pressed() {
        if c == 3 as char || c == 13 as char {
            if player_name.len() > 2 {
                *status = Status::SelectMap;
                return;
            }
        }
        if player_name.len() < MAX_NAME_LENGTH && is_valid_name_char(c) {
            player_name.push(c);
        }
    }

    if is_key_pressed(KeyCode::Backspace) {
        player_name.pop();
    }

    if is_key_pressed(KeyCode::Escape) {
        exit(0);
    }
}
fn select_map_handler(
    status: &mut Status,
    map_path: &mut String,
    server_addr: &String,
    player_name: &String,
    selected_path_index: &mut i32,
) {
    if let Ok(paths) = fs::read_dir(MAPS_DIRECTORY_PATH) {
        let mut map_paths = vec![];
        for path in paths {
            if let Ok(_path) = path {
                map_paths.push(_path.path());
            }
        }
        if map_paths.len() == 0 {
            *status = Status::Init;
            return;
        }

        draw_text(
            format!(
                "Enter server IP addrsss. Example: 127.0.0.1:4000    {}",
                server_addr
            )
            .as_str(),
            10.0,
            20.0,
            CONSOLE_FONT_SIZE,
            LIGHTGRAY,
        );
        draw_text(
            format!("Enter your name:     {}", player_name).as_str(),
            10.0,
            40.0,
            CONSOLE_FONT_SIZE,
            LIGHTGRAY,
        );

        let mut off_set_y = 70.0;
        for (index, path) in map_paths.iter().enumerate() {
            let mut text = format!("{:?}", path.display());
            text = text.replace('"', "");
            if index as i32 == *selected_path_index {
                draw_rectangle(
                    0.0,
                    off_set_y - 5.0 - 12.0,
                    screen_width(),
                    CONSOLE_FONT_SIZE + 5.0,
                    LIGHTGRAY,
                );
                draw_text(text.as_str(), 10.0, off_set_y, CONSOLE_FONT_SIZE, BLACK);
            } else {
                draw_text(text.as_str(), 10.0, off_set_y, CONSOLE_FONT_SIZE, LIGHTGRAY);
            }
            off_set_y += 30.0;
        }

        if is_key_pressed(KeyCode::Down) {
            *selected_path_index =
                i32::min(map_paths.len() as i32 - 1, selected_path_index.clone() + 1);
        }
        if is_key_pressed(KeyCode::Up) {
            *selected_path_index = i32::max(0, selected_path_index.clone() - 1);
        }

        if let Some(c) = get_char_pressed() {
            if c == 3 as char || c == 13 as char {
                *map_path = format!("{}", map_paths[*selected_path_index as usize].display());
                *status = Status::Init;
            }
        }
        if is_key_pressed(KeyCode::Escape) {
            exit(0);
        }
    } else {
        *status = Status::Init;
    }
}
fn init_game_handler(
    status: &mut Status,
    game_params: &mut Option<GameParams>,
    player: &mut Option<Arc<Mutex<Player>>>,
    player_name: &String,
    map_path: &String,
) {
    let params = init_game_params(map_path);
    *game_params = Some(params.clone());
    let _player = init_player(&params, player_name, map_path);
    *player = Some(Arc::new(Mutex::new(_player)));
    *status = Status::StartServerListener;
}
fn handle_game_run(
    server_addr: &String,
    player_ref: Arc<Mutex<Player>>,
    game_params: &mut GameParams,
    socket: &Arc<UdpSocket>,
    enemies: Arc<Mutex<Option<Vec<Player>>>>,
    is_first_tun: &mut bool,
    fps: f32,
    font: &Font,
    move_speed: f32,
    look_speed: f32,
) {
    let mut require_update = false;
    if *is_first_tun {
        *is_first_tun = false;
        require_update = true;
    }

    let delta = get_frame_time();

    match player_ref.lock() {
        Ok(mut player) => {
            let prev_pos = player.position_vec3.clone();
            let front = player.clone().front;
            let right = player.clone().right;

            if is_key_pressed(KeyCode::Escape) {
                player.player_status = PlayerStatus::Disconnent;
                send_message_to_server(socket, server_addr, player.clone(), player.id.clone());
                exit(0);
            }
            if is_key_down(KeyCode::Up) || is_key_down(KeyCode::W) {
                player.position_vec3 += front * move_speed;
                require_update = true;
            }
            if is_key_down(KeyCode::Down) || is_key_down(KeyCode::S) {
                player.position_vec3 -= front * move_speed;
                require_update = true;
            }
            if is_key_down(KeyCode::Left) || is_key_down(KeyCode::A) {
                player.position_vec3 -= right * move_speed;
                require_update = true;
            }
            if is_key_down(KeyCode::Right) || is_key_down(KeyCode::D) {
                player.position_vec3 += right * move_speed;
                require_update = true;
            }

            let gap: f32 = 0.05;
            handle_wall_collisions(
                &game_params.mini_map,
                prev_pos,
                &mut player.position_vec3,
                gap,
            );
            let mouse_position: Vec2 = mouse_position().into();
            let mouse_delta = mouse_position - game_params.last_mouse_position;
            let mouse_delta_length = mouse_delta.length();
            if mouse_delta_length > 0.0 {
                require_update = true;
            }
            game_params.last_mouse_position = mouse_position;

            player.yaw += mouse_delta.x * delta * look_speed;
            player.pitch += mouse_delta.y * delta * -look_speed;
            player.pitch = if player.pitch > MAX_PITCH {
                MAX_PITCH
            } else {
                player.pitch
            };
            player.pitch = if player.pitch < MIN_PITCH {
                MIN_PITCH
            } else {
                player.pitch
            };
            player.front = vec3(
                player.yaw.cos() * player.pitch.cos(),
                player.pitch.sin(),
                player.yaw.sin() * player.pitch.cos(),
            )
            .normalize();
            player.right = player.front.cross(game_params.world_up).normalize();
            let up = player.right.cross(player.front).normalize();
            player.position_vec3.y = PLAYER_HEIGHT;
            //2d
            set_default_camera();
            clear_background(WHITE);
            player.position = Position::build(player.position_vec3.x, player.position_vec3.z);
            //find projection of front on x_z plane
            let p = player.front.dot(game_params.world_up) * game_params.world_up;
            let orientation = (player.front - p).normalize();
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

            let params = TextParams {
                font: Some(font),
                font_size: GAME_FONT_SIZE,
                font_scale: 1.0,
                font_scale_aspect: 1.0,
                rotation: 0.0,
                color: BLACK,
            };
            draw_text_ex(
                &player.name,
                NAME_MARGIN_LEFT as f32,
                NAME_MARGIN_TOP as f32,
                params.clone(),
            );
            draw_text_ex(
                format!("{}", player.score).as_str(),
                SCORE_MARGIN_LEFT as f32,
                NAME_MARGIN_TOP as f32,
                params,
            );

            draw_text(
                format!("FPS: {:.1$}", fps, 2).as_str(),
                FPS_MARGIN_LEFT as f32,
                FPS_MARGIN_TOP as f32,
                CONSOLE_FONT_SIZE,
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
                    draw_enemy_names_and_scores(&enemies, font);
                    draw_enemies_on_minimap(&enemies, &game_params);
                }
            }

            set_camera(&Camera3D {
                render_target: Some(game_params.render_target.clone()),
                position: player.position_vec3,
                up: up,
                target: player.position_vec3 + player.front,
                ..Default::default()
            });

            clear_background(LIGHTGRAY);
            //draw_grid(50, 1., BLACK, GRAY);
            draw_walls(
                &game_params.mini_map,
                Some(&game_params.wall_texture),
                WHITE,
            );
            //sky
            // for z in 0..game_params.mini_map.len(){
            //     for x in 0..game_params.mini_map[0].len(){
            //         draw_plane(vec3(x as f32, 1.5, z as f32), vec2(1.0,1.0), Some(&game_params.sky_texture), WHITE);
            //     }
            // }
            let center = vec3(0.0, 1.5, 0.0);
            let size = vec2(game_params.mini_map[0].len() as f32, game_params.mini_map.len() as f32);
            draw_plane(center, size, None, Color{r: 0.3, g: 0.79, b: 0.99, a: 0.7});
            
            //ground
            for z in 0..game_params.mini_map.len(){
                for x in 0..game_params.mini_map[0].len(){
                    draw_plane(vec3(x as f32, 0.5, z as f32), vec2(1.0,1.0), Some(&game_params.floor_texture), WHITE);
                }
            }

            //let center = vec3(-50.0, -0.1, -50.0);
            //let size = vec2(100.0, 100.0);
            //draw_plane(center, size, Some(&game_params.floor_texture), WHITE);

            //draw enemies in 3D window
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
                    Ok(mut hittables) => {
                        let mut closest_hit_option: Option<Hit> = None;

                        let start =
                            vec3(player.position.x, 0.95, player.position.z) + player.front / 10.0;

                        for hittable in hittables.iter() {
                            if let Hittable::Wall(shield) = hittable {
                                let hit_option = shield.hit(start, player.front);
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
                            if let Hittable::Enemy(enemy) = hittable {
                                let hit_option = enemy.hit(start, player.front);
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
                                Hittable::Wall(_) => {}
                                Hittable::Enemy(mut enemy) => {
                                    //hit enemy
                                    //update score
                                    if let PlayerStatus::Active = enemy.player_status {
                                        player.score = player.score + 1;
                                        require_update = true;
                                    }
                                    enemy.player_status = PlayerStatus::Killed;

                                    //remove from hitables
                                    let _hittables: Vec<Hittable> = hittables
                                        .iter()
                                        .filter(|hittable| {
                                            if let Hittable::Enemy(_enemy) = hittable {
                                                _enemy.id != enemy.id
                                            } else {
                                                true
                                            }
                                        })
                                        .cloned()
                                        .collect();
                                    *hittables = _hittables;

                                    //notify server
                                    send_message_to_server(
                                        socket,
                                        server_addr,
                                        enemy.clone(),
                                        player.id.clone(),
                                    );
                                }
                            }
                            closest_hit.p
                        } else {
                            start + player.front * MAX_SHOT_RANGE
                        };

                        let shot = Shot {
                            start,
                            end,
                            time_out: SHOT_DURATION,
                            color: YELLOW,
                        };
                        game_params.shots.push(shot);
                    }
                    Err(e) => println!("Error while locking hittables {:?}", e),
                }
            }

            draw_shots(&game_params.shots);
            remove_shots(&mut game_params.shots);

            if require_update {
                send_message_to_server(socket, server_addr, player.clone(), player.id.clone());
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
    server_addr: String,
) {
    let player_id = player.lock().unwrap().id.clone();
    //Server response listener
    thread::spawn(move || loop {
        let mut buffer = [0u8; 2048];
        if let Ok((size, _)) = socket.recv_from(&mut buffer) {
            // println!(
            //     "Received {} bytes from {}: {}",
            //     size,
            //     src,
            //     std::str::from_utf8(&buffer[..size]).unwrap_or("<invalid UTF-8>")
            // );

            if let Ok(players_str) = std::str::from_utf8(&buffer[..size]) {
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
                                            player_locked.position_vec3 = position;
                                            player_locked.position =
                                                Position::build(position.x, position.z);
                                            player_locked.player_status = PlayerStatus::Active;
                                            //send message to server
                                            let server_object = ServerMessage {
                                                sender_id: player_locked.id.clone(),
                                                player: player_locked.clone(),
                                            };
                                            if let Ok(message_to_server) =
                                                serde_json::to_string(&server_object)
                                            {
                                                if let Err(e) = socket.send_to(
                                                    message_to_server.as_bytes(),
                                                    server_addr.clone(),
                                                ) {
                                                    println!(
                                                        "Error while sending message {} to server: {:?}",
                                                        message_to_server, e
                                                    );
                                                }
                                            }
                                        }
                                        Err(e) => println!("Error while locking player: {:?}", e),
                                    }
                                }
                            } else {
                                //collect enemies
                                if let PlayerStatus::Active = _player.player_status {
                                    if let Some(ref mut enemies_local) = enemies_local_option {
                                        enemies_local.push(_player.clone());
                                    } else {
                                        enemies_local_option = Some(vec![_player.clone()]);
                                    }
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
fn draw_enemy_names_and_scores(_enemies: &Vec<Player>, font: &Font) {
    let mut top_offset = NAME_MARGIN_TOP as f32 + 25.0;
    let params = TextParams {
        font: Some(font),
        font_size: GAME_FONT_SIZE,
        font_scale: 1.0,
        font_scale_aspect: 1.0,
        rotation: 0.0,
        color: BLACK,
    };

    let mut enemies = _enemies.clone();
    enemies.sort_by(|a, b| b.score.cmp(&a.score));
    if enemies.len() > 8 {
        enemies =  enemies[0..8].to_vec();
    }
    
    for enemy in enemies {
        if let PlayerStatus::Active = enemy.player_status {
            draw_text_ex(
                &enemy.name,
                NAME_MARGIN_LEFT as f32,
                top_offset,
                params.clone(),
            );
            draw_text_ex(
                format!("{}", enemy.score).as_str(),
                SCORE_MARGIN_LEFT as f32,
                top_offset,
                params.clone(),
            );
            top_offset += 25.0;
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
fn send_message_to_server(
    socket: &Arc<UdpSocket>,
    server_addr: &String,
    player: Player,
    sender_id: String,
) {
    let server_object = ServerMessage { sender_id, player };
    if let Ok(message_to_server) = serde_json::to_string(&server_object) {
        //println!("message to server: {:?}", message_to_server);
        //println!();
        if let Err(e) = socket.send_to(message_to_server.as_bytes(), server_addr) {
            println!(
                "Error while sending message {} to server: {:?}",
                message_to_server, e
            );
        }
    }
}
fn draw_enemies_test(font: &Font) {
    let mut p1 = Player::new();
    p1.name = "AAA".to_string();
    p1.score = 2;
    p1.player_status = PlayerStatus::Active;
    let mut p2 = Player::new();
    p2.name = "BBB".to_string();
    p2.score = 0;
    p2.player_status = PlayerStatus::Active;
    let mut p3 = Player::new();
    p3.name = "CCC".to_string();
    p3.score = 1;
    p3.player_status = PlayerStatus::Active;
    let mut p4 = Player::new();
    p4.name = "DDD".to_string();
    p4.score = 3;
    p4.player_status = PlayerStatus::Active;
    let mut p5 = Player::new();
    p5.name = "EEE".to_string();
    p5.score = 0;
    p5.player_status = PlayerStatus::Active;
    let mut p6 = Player::new();
    p6.name = "FFF".to_string();
    p6.score = 5;
    p6.player_status = PlayerStatus::Active;
    let mut p7 = Player::new();
    p7.name = "GGG".to_string();
    p7.score = 3;
    p7.player_status = PlayerStatus::Active;
    let mut p8 = Player::new();
    p8.name = "HHH".to_string();
    p8.score = 3;
    p8.player_status = PlayerStatus::Active;
    let mut p9 = Player::new();
    p9.name = "III".to_string();
    p9.score = 8;
    p9.player_status = PlayerStatus::Active;

    let mut enemies = vec![p1, p2, p3, p4, p5, p6, p7, p8, p9];

    enemies.sort_by(|a, b| b.score.cmp(&a.score));
    let enemies = enemies[0..8].to_vec();

    let mut top_offset = NAME_MARGIN_TOP as f32 + 25.0;
    let params = TextParams {
        font: Some(font),
        font_size: GAME_FONT_SIZE,
        font_scale: 1.0,
        font_scale_aspect: 1.0,
        rotation: 0.0,
        color: BLACK,
    };

    for enemy in enemies {
        if let PlayerStatus::Active = enemy.player_status {
            draw_text_ex(
                &enemy.name,
                NAME_MARGIN_LEFT as f32,
                top_offset,
                params.clone(),
            );
            draw_text_ex(
                format!("{}", enemy.score).as_str(),
                SCORE_MARGIN_LEFT as f32,
                top_offset,
                params.clone(),
            );
            top_offset += 25.0;
        }
    }
}
