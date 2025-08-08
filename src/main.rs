use macroquad::prelude::*;
use std::{io, process::exit, usize};
use serde_json::from_str;
use std::sync::Mutex;

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
    let mut player = Player::new();
    player.current_map = String::from("map_one");
    let mut status = Status::EnterIP;
    request_new_screen_size(SCREEN_WIDTH as f32, SCREEN_HEIGHT as f32);
    let mut server_addr = String::new();
    //init game engine
    let mut game_params = init_game_params();
    set_cursor_grab(true);
    show_mouse(false);
    //let mut enemies: HashMap<String, Player> = HashMap::new();
    let enemies: Arc<Mutex<Option<Vec<Player>>>> = Arc::new(Mutex::new(None));
    let socket = Arc::new(UdpSocket::bind("0.0.0.0:0").unwrap()); 
    start_server_listener(Arc::clone(&socket), Arc::clone(&enemies));
    loop {
        match status {
            Status::EnterIP => {
                handle_ip_input(&mut status, &mut server_addr)
            }
            Status::EnterName =>{
               handle_name_input(&mut status, &mut player, &server_addr);
            },
            Status::Run => handle_game_run(&server_addr, &mut player, &mut game_params, &socket,  Arc::clone(&enemies)),
        }
        next_frame().await;
    }
}
fn init_game_params() -> GameParams{
    let wall_texture = Texture2D::from_file_with_format(include_bytes!("../assets/grey.png"), None);
    let sky_texture = Texture2D::from_file_with_format(include_bytes!("../assets/sky.png"), None);
    let up_texture = Texture2D::from_file_with_format(include_bytes!("../assets/up_180.png"), None);
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
    
    GameParams{
        wall_texture,
        sky_texture,
        up_texture,        
        mini_map_config,
        render_target,
        yaw, 
        pitch, 
        front,
        right, 
        position, 
        last_mouse_position,
        mini_map,
        world_up
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
    vec3(x, 1.0, z) 
}
fn draw_player_on_mini_map(
    player: &Player,
    mini_map: &Vec<Vec<bool>>,
    config: &MiniMapConfig,
    up_texture: &Texture2D
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
fn draw_enemy_on_minimap(player: &Player, mini_map: &Vec<Vec<bool>>, config: &MiniMapConfig, color: Color){
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
  draw_circle(x + image_size/2.0, z+image_size/2.0, image_size/2.5, color);
}
fn draw_walls(mini_map: &Vec<Vec<bool>>, texture: Option<&Texture2D>, color: Color) {
    for (z, line) in mini_map.into_iter().enumerate() {
        for (x, cell) in line.iter().enumerate() {
            if *cell {
                let position = vec3(x as f32, 1.0, z as f32);
                let size = vec3(1.0, 1.0, 1.0);
                //draw_cube_wires(vec3(x as f32, 1.0, z as f32), vec3(x as f32 + 1.0, 2.0, z as f32 + 1.0), BLACK);
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
fn handle_ip_input(
    status: &mut Status,
    server_addr: &mut String,
) {
    clear_background(BLACK);
    let mut server_addr_display = "Enter server IP addrsss. Example: 127.0.0.1:4000    ".to_string();
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
fn handle_name_input(
    status: &mut Status,
    player: &mut Player,
    server_addr: &String,
) {
    clear_background(BLACK);
    let mut server_addr_display = "Enter server IP addrsss. Example: 127.0.0.1:4000    ".to_string();
    server_addr_display.push_str(server_addr);

    let mut player_name_display = "Enter your name:     ".to_string();
    player_name_display.push_str(&player.name);

    draw_text(server_addr_display.as_str(), 10.0, 20.0, 20.0, LIGHTGRAY);
    draw_text(player_name_display.as_str(), 10.0, 40.0, 20.0, LIGHTGRAY);

    if let Some(c) = get_char_pressed() {
        if c == 3 as char || c == 13 as char {
            if player.name.len() > 2{
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

    if is_key_pressed(KeyCode::Escape) {
        exit(0);
    }
}
fn handle_game_run(server_addr: &String, player: &mut Player, game_params: &mut GameParams, socket: &Arc<UdpSocket>, enemies: Arc<Mutex<Option<Vec<Player>>>>){
    let delta = get_frame_time();
    let prev_pos = game_params.position.clone();
    if is_key_pressed(KeyCode::Escape) {
       player.is_active = false;
       if let Ok(message_to_server) = serde_json::to_string(player){
          let _ = socket.send_to(message_to_server.as_bytes(), server_addr);
       }     
       exit(0);
    }
    if is_key_down(KeyCode::Up) || is_key_down(KeyCode::W) {
        game_params.position +=  game_params.front * MOVE_SPEED;
    }
    if is_key_down(KeyCode::Down) || is_key_down(KeyCode::S) {
        game_params.position -=  game_params.front * MOVE_SPEED;
    }
    if is_key_down(KeyCode::Left) || is_key_down(KeyCode::A) {
         game_params.position -=  game_params.right * MOVE_SPEED;
    }
    if is_key_down(KeyCode::Right) || is_key_down(KeyCode::D) {
         game_params.position +=  game_params.right * MOVE_SPEED;
    }
    let gap: f32 = 0.05;
    handle_wall_collisions(&game_params.mini_map, prev_pos, &mut  game_params.position, gap);
    let mouse_position: Vec2 = mouse_position().into();
    let mouse_delta = mouse_position -  game_params.last_mouse_position;
    game_params.last_mouse_position = mouse_position;
    game_params.yaw += mouse_delta.x * delta * LOOK_SPEED;
    game_params.pitch += mouse_delta.y * delta * -LOOK_SPEED;
    game_params.pitch = if game_params.pitch > 0.35 { 0.35 } else { game_params.pitch };
    game_params.pitch = if game_params.pitch < -0.35 { -0.35 } else {  game_params.pitch };
    game_params.front = vec3(
         game_params.yaw.cos() *  game_params.pitch.cos(),
         game_params.pitch.sin(),
         game_params.yaw.sin() *  game_params.pitch.cos(),
    )
    .normalize();
     game_params.right =  game_params.front.cross(game_params.world_up).normalize();
    let up =  game_params.right.cross( game_params.front).normalize();
    game_params.position.y = 1.0;
    //2d
    set_default_camera();
    clear_background(WHITE);
    player.position = Position::build(game_params.position.x,  game_params.position.z);
    //find projection of front on x_z plane
    let p =  game_params.front.dot(game_params.world_up) *  game_params.world_up;
    let orientation = ( game_params.front - p).normalize();
    player.orientation = orientaion_to_degrees(vec3(orientation.x, orientation.y, orientation.z));
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
    render_mini_map(& game_params.mini_map, &game_params.mini_map_config);
    draw_player_on_mini_map(&player, & game_params.mini_map, & game_params.mini_map_config, & game_params.up_texture);
    draw_texture_ex(
        & game_params.render_target.texture,
        MAIN_MARGIN_LEFT as f32,
        (MAIN_MARGIN_TOP + MAIN_HEIGHT) as f32,
        WHITE,
        DrawTextureParams {
            dest_size: Some(Vec2::new(MAIN_WIDTH as f32, -1.0 * MAIN_HEIGHT as f32)),
            ..Default::default()
        },
    );
    //enemies
    if let Ok(enemies_result) = enemies.lock(){
        if let Some(enemies) = enemies_result.clone(){
            draw_enemies_on_minimap(&enemies, &game_params);        
        }
    }

    //3d
    set_camera(&Camera3D {
        render_target: Some( game_params.render_target.clone()),
        position:  game_params.position,
        up: up,
        target: game_params.position +  game_params.front,
        ..Default::default()
    });
    clear_background(LIGHTGRAY);
    draw_grid(50, 1., BLACK, GRAY);
    draw_walls(& game_params.mini_map, Some(& game_params.wall_texture), WHITE);
    //sky
    let center = vec3(-20.0, 5.0, -20.0);
    let size = vec2(100.0, 100.0);
    draw_plane(center, size, Some(& game_params.sky_texture), WHITE);
    //ground
    let center = vec3(-50.0, -0.1, -50.0);
    let size = vec2(100.0, 100.0);
    draw_plane(center, size, None, BROWN);

    //enemies
    if let Ok(enemies_result) = enemies.lock(){
        if let Some(enemies) = enemies_result.clone(){
            for enemy in enemies{
                draw_sphere(vec3(enemy.position.x, 1.0, enemy.position.z), 0.2, None, PURPLE);
            }        
        }
    }

    if let Ok(message_to_server) = serde_json::to_string(player){
        let _ = socket.send_to(message_to_server.as_bytes(), server_addr);
    } 

}
fn start_server_listener(socket: Arc<UdpSocket>, enemies: Arc<Mutex<Option<Vec<Player>>>>){
    //Server response listener
     thread::spawn(move | | loop {
        let mut buffer = [0u8; 1024];
        if let Ok((size, _)) = socket.recv_from(&mut buffer){
            // println!(
            //     "Received {} bytes from {}: {}",
            //     size,
            //     src,
            //     std::str::from_utf8(&buffer[..size]).unwrap_or("<invalid UTF-8>")
            // );
            if let Ok(enemies_str) = std::str::from_utf8(&buffer[..size]){                
                let enemies_result = from_str::<Vec<Player>>(enemies_str);
                match enemies_result {
                    Ok(es) => {   
                        let enemies_locked_result =  enemies.lock();
                        match enemies_locked_result{
                            Ok(mut enemies_locked) => *enemies_locked = Some(es),
                            Err(e) => println!("Error while locking: {e}")
                        }
                    },
                    Err(e) => println!("Error Parsing: {e}")                    
                }
            }else {
                println!("no enemies...",);
                let enemies_locked_result =  enemies.lock();
                match enemies_locked_result{
                    Ok(mut enemies_locked) => *enemies_locked = None,
                    Err(e) => println!("Error while locking: {e}")
                }
            }

        }
     });
}
fn draw_enemies_on_minimap(enemies: &Vec<Player>, game_params: &GameParams){

    let mut top_offset = NAME_MARGIN_TOP as f32;

    for enemy in enemies {
        if enemy.is_active{
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

            draw_enemy_on_minimap(&enemy, &game_params.mini_map, &game_params.mini_map_config, RED);

            //draw enemy in 3d



        }  
    } 



}

/*
use macroquad::prelude::*;
use std::net::UdpSocket;
use std::process::exit;
use std::sync::Arc;
use std::thread;
mod preferences;
use preferences::*;

mod models;
use models::*;

#[macroquad::main("FPS-CLIENT")]
async fn main() {
   let mut player = Player::new();

   let mut status = Status::EnterIP;

   request_new_screen_size(SCREEN_WIDTH as f32, SCREEN_HEIGHT as f32);


    //send request from client:  echo -n 'Hello from client' | nc -u 127.0.0.1 4000

    //  let socket = Arc::new(UdpSocket::bind("0.0.0.0:0").unwrap());
    //  let socket_in_thread = Arc::clone(&socket);

    //  let mut server_addr = String::new();
    //  println!("Enter server IP addrsss. Example: 127.0.0.1:4000 ");
    //  io::stdin()
    //      .read_line(&mut server_addr)
    //      .expect("Failed to read line");

    //  let mut name = String::new();
    //  println!("Enter your name ");
    //  io::stdin()
    //      .read_line(&mut name)
    //      .expect("Failed to read line");

    //  //Server response listener
    //  thread::spawn(move || loop {
    //      let mut buffer = [0u8; 1024];
    //      let (size, src) = socket_in_thread.recv_from(&mut buffer).unwrap();
    //      println!(
    //          "Received {} bytes from {}: {}",
    //          size,
    //          src,
    //          str::from_utf8(&buffer[..size]).unwrap_or("<invalid UTF-8>")
    //      );
    //  });

    let mut server_addr = String::new();

    loop {
        match status {
            Status::EnterIP => {
                handle_ip_input(&mut status, &mut server_addr)
            }
            Status::EnterName =>{
               handle_name_input(&mut status, &mut player, &server_addr);
            },
            Status::Run => handle_game_run(&server_addr, &mut player),
        }
        next_frame().await
    }
}



fn is_valid_ip_char(c: char) -> bool {
    if c >= '0' && c <= '9' {
        return true;
    }
    if c == '.' || c == ':' {
        return true;
    }
    false
}

fn is_valid_name_char(c: char) -> bool {
    c >= ' ' && c <= '~'
}

fn handle_ip_input(
    status: &mut Status,
    server_addr: &mut String,
) {
    clear_background(BLACK);
    let mut server_addr_display = "Enter server IP addrsss. Example: 127.0.0.1:4000    ".to_string();
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

fn handle_name_input(
    status: &mut Status,
    player: &mut Player,
    server_addr: &String,
) {
    clear_background(BLACK);
    let mut server_addr_display = "Enter server IP addrsss. Example: 127.0.0.1:4000    ".to_string();
    server_addr_display.push_str(server_addr);

    let mut player_name_display = "Enter your name:     ".to_string();
    player_name_display.push_str(&player.name);

    draw_text(server_addr_display.as_str(), 10.0, 20.0, 20.0, LIGHTGRAY);
    draw_text(player_name_display.as_str(), 10.0, 40.0, 20.0, LIGHTGRAY);

    if let Some(c) = get_char_pressed() {
        if c == 3 as char || c == 13 as char {
            if player.name.len() > 2{
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

    if is_key_pressed(KeyCode::Escape) {
        exit(0);
    }
}

fn handle_game_run(server_addr: &String, player: &mut Player) {
    draw_ui(&player);

    let socket = Arc::new(UdpSocket::bind("0.0.0.0:0").unwrap());

    //Exit game handler
    if is_key_pressed(KeyCode::Escape) {
        player.is_active = false;
        let message_to_server = serde_json::to_string(player).unwrap();
        if let Ok(_) = socket.send_to(message_to_server.as_bytes(), server_addr) {
            println!("Player pressed LEFT");
        } else {
            println!("Error while sending message to server");
        }
        exit(0);
    }

    if is_key_pressed(KeyCode::A) || is_key_pressed(KeyCode::Left) {
        player.position.x -= 1.0;
        let message_to_server = serde_json::to_string(player).unwrap();
        if let Ok(_) = socket.send_to(message_to_server.as_bytes(), server_addr) {
            println!("Player pressed LEFT");
        } else {
            println!("Error while sending message to server");
        }
    }

    if is_key_pressed(KeyCode::D) || is_key_pressed(KeyCode::Right) {
        player.position.x += 1.0;
        let message_to_server = serde_json::to_string(player).unwrap();
        if let Ok(_) = socket.send_to(message_to_server.as_bytes(), server_addr) {
            println!("Player pressed RIGHT");
        } else {
            println!("Error while sending message to server");
        }
    }

    if is_key_pressed(KeyCode::W) || is_key_pressed(KeyCode::Up) {
        player.position.y -= 1.0;
        let message_to_server = serde_json::to_string(player).unwrap();
        if let Ok(_) = socket.send_to(message_to_server.as_bytes(), server_addr) {
            println!("Player pressed UP");
        } else {
            println!("Error while sending message to server");
        }
    }

    if is_key_pressed(KeyCode::S) || is_key_pressed(KeyCode::Down) {
        player.position.y += 1.0;
        let message_to_server = serde_json::to_string(player).unwrap();
        if let Ok(_) = socket.send_to(message_to_server.as_bytes(), server_addr) {
            println!("Player pressed DOWN");
        } else {
            println!("Error while sending message to server");
        }
    }
}

fn draw_ui(player: &Player){
   clear_background(WHITE);
   draw_rectangle_lines(MAIN_MARGIN_LEFT as f32, MAIN_MARGIN_TOP as f32, MAIN_WIDTH as f32, MAIN_HEIGHT as f32, 2.0, DARKGRAY);
   draw_rectangle_lines(MAP_MARGIN_LEFT as f32, MAP_MARGIN_TOP as f32, MAP_WIDTH as f32, MAP_HEIGHT as f32, 2.0,  DARKGRAY);
   draw_text(&player.name, NAME_MARGIN_LEFT as f32, NAME_MARGIN_TOP as f32, 20.0, DARKGRAY);
   draw_text(format!("{}", player.score).as_str(), SCORE_MARGIN_LEFT as f32, NAME_MARGIN_TOP as f32, 20.0, DARKGRAY);
}
   */
