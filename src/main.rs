use macroquad::{prelude::*, text, texture};
use std::{f64::consts::PI, io, process::exit, usize};

mod preferences;
use preferences::*;

mod models;
use models::*;

mod utils;
use utils::*;

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

    let texture = Texture2D::from_file_with_format(
    include_bytes!("../assets/grey.png"),
    None,
    );

    let sky_texture = Texture2D::from_file_with_format(
    include_bytes!("../assets/sky.png"),
    None,
    );

    let up_texture = Texture2D::from_file_with_format(
    include_bytes!("../assets/up_200.png"),
    None,
    );

    let mini_map = match  parse_map("assets/map_one.txt") {
        Ok(map) => map,
        Err(error) => {
            println!("Problem opening the file: {error:?}");
            exit(1);
        },
    };

    let mini_map_config = MiniMapConfig::new(&mini_map, MAP_WIDTH, MAP_HEIGHT, MAP_MARGIN_LEFT, MAP_MARGIN_TOP, BLACK);

    let render_target = render_target_ex(MAIN_WIDTH, MAIN_HEIGHT, RenderTargetParams{sample_count: 1, depth: true});

    let world_up = vec3(0.0, 1.0, 0.0);
    let mut yaw: f32 = 1.18; //rotation around y axes   
    let mut pitch: f32 = 0.0; //tilt up/down

    let mut front = vec3(
        yaw.cos() * pitch.cos(),
        pitch.sin(),
        yaw.sin() * pitch.cos(),
    )
    .normalize();
    let mut right = front.cross(world_up).normalize();



    //let mut position = vec3(0.0, 1.0, 0.0);
    let mut position = generate_position(&mini_map);
    let mut last_mouse_position: Vec2 = mouse_position().into();

    set_cursor_grab(true);
    show_mouse(false);

    loop {     
        
        let delta = get_frame_time();

         let prev_pos = position.clone();

        if is_key_pressed(KeyCode::Escape) {
            break;
        }
        if is_key_down(KeyCode::Up) {
            position += front * MOVE_SPEED;
        }
        if is_key_down(KeyCode::Down) {
            position -= front * MOVE_SPEED;
        }
        if is_key_down(KeyCode::Left) {
            position -= right * MOVE_SPEED;
        }
        if is_key_down(KeyCode::Right) {
            position += right * MOVE_SPEED;
        }


        let gap: f32 = 0.05;
        handle_wall_collisions(&mini_map, prev_pos, &mut position, gap);      


        let mouse_position: Vec2 = mouse_position().into();
        let mouse_delta = mouse_position - last_mouse_position;      
        last_mouse_position = mouse_position;
        
        yaw += mouse_delta.x * delta * LOOK_SPEED;
        pitch += mouse_delta.y * delta * -LOOK_SPEED;

        pitch = if pitch > 0.35 { 0.35 } else { pitch };
        pitch = if pitch < -0.35 { -0.35 } else { pitch };

        front = vec3(
            yaw.cos() * pitch.cos(),
            pitch.sin(),
            yaw.sin() * pitch.cos(),
        )
        .normalize();


        right = front.cross(world_up).normalize();
        let up = right.cross(front).normalize();

        position.y = 1.0;   

        //2d        
        set_default_camera();
        clear_background(WHITE);
        let mut player = Player::new();
        player.name = "Alex".to_string();
        player.position = Position::build(position.x, position.z);
        //find projection of front on x_z plane
        let p = front.dot(world_up)*world_up;
        let orientation = (front - p).normalize();
        player.orientation = Orientation::new(orientation.x, orientation.y, orientation.z);
        draw_rectangle_lines(MAP_MARGIN_LEFT as f32, MAP_MARGIN_TOP as f32, MAP_WIDTH as f32, MAP_HEIGHT as f32, 2.0,  DARKGRAY);
        draw_text(&player.name, NAME_MARGIN_LEFT as f32, NAME_MARGIN_TOP as f32, 20.0, DARKGRAY);
        draw_text(format!("{}", player.score).as_str(), SCORE_MARGIN_LEFT as f32, NAME_MARGIN_TOP as f32, 20.0, DARKGRAY);
        render_mini_map(&mini_map, &mini_map_config);
        draw_player(&player, &mini_map, &mini_map_config, &up_texture, PURPLE); 
        draw_texture_ex(&render_target.texture, MAIN_MARGIN_LEFT as f32, (MAIN_MARGIN_TOP + MAIN_HEIGHT) as f32, WHITE, DrawTextureParams{
            dest_size: Some(Vec2::new(MAIN_WIDTH as f32, -1.0 * MAIN_HEIGHT as f32)),
            ..Default::default()
        });
        
        //3d     
        set_camera(&Camera3D {
            render_target: Some(render_target.clone()),
            position: position,
            up: up,
            target: position + front,
            ..Default::default()
        });
        clear_background(LIGHTGRAY);
        draw_grid(50, 1., BLACK, GRAY);
        //draw_cube_wires(vec3(0.0, 2.0, 0.0), vec3(5., 5., 5.), DARKGREEN);
        //draw_cube_wires(vec3(0., 1., -6.0), vec3(2., 2., 2.), GREEN);
        //draw_cube_wires(vec3(0., 1., 6.), vec3(2., 2., 2.), BLUE);
        //draw_cube_wires(vec3(2., 1., 2.), vec3(2., 2., 2.), RED);

        draw_walls(&mini_map, Some(&texture), WHITE);

        //sky
        let center = vec3(-20.0, 5.0, -20.0);
        let size = vec2(100.0, 100.0);   
        draw_plane(center, size, Some(&sky_texture), WHITE);
        
        //ground
        let center = vec3(-50.0, -0.1, -50.0);
        let size = vec2(100.0, 100.0);   
        draw_plane(center, size, None, BROWN);
        
        next_frame().await
    }
}


fn parse_map(file_path: &str) -> Result<Vec<Vec<bool>>, io::Error>{
    let content = read_file(file_path)?;
    if !is_map_valid(&content){
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid Map Format"));
    }
    Ok(map_to_slice(&content))
}
fn render_mini_map(mini_map: &Vec<Vec<bool>>, mini_map_config: &MiniMapConfig){
    let mut horizontal_offset: f32 = mini_map_config.horizontal_offset as f32;
    let mut vertical_offset: f32 = mini_map_config.vertical_offset as f32;
    for line in mini_map{        
        for cell in line{
            if *cell{
                draw_rectangle(horizontal_offset, vertical_offset, mini_map_config.cell_width, mini_map_config.cell_height, mini_map_config.cell_color);
            }
            horizontal_offset += mini_map_config.cell_width
        }
        horizontal_offset = mini_map_config.horizontal_offset as f32;
        vertical_offset += mini_map_config.cell_height;
    }    
}
fn generate_position(map: &Vec<Vec<bool>>) -> Vec3{
    let mut spaces: Vec<(usize, usize)> = vec![];
    for (z, line) in map.iter().enumerate(){
        for (x, cell) in line.iter().enumerate(){
            if !*cell{
                spaces.push((x, z));
            }
        }
    }
    let rand_index = generate_up_to(spaces.len());
    let x = spaces[rand_index].0 as f32;
    let z = spaces[rand_index].1 as f32;
    vec3(x, 1.0, z)
    
}
fn draw_player(player: &Player, mini_map: &Vec<Vec<bool>>, config: &MiniMapConfig, up_texture: &Texture2D, color: Color){
    let radius = f32::min(config.cell_width, config.cell_height)/2.5;
    let mut x = config.horizontal_offset as f32 + player.position.x*config.cell_width + config.cell_width/2.0;
    let mut z = config.vertical_offset as f32 + player.position.z*config.cell_height + config.cell_height/2.0;
    //draw_circle(x, z, radius, color);

    let image_size = f32::min(config.cell_width, config.cell_height);
    x -= image_size/2.0;
    z -= image_size/2.0;

    
    /*
    //current cell indecies
    let current_x =f32::floor(player.position.x + 0.5);
    let current_z =f32::floor(player.position.z + 0.5);
    //println!("player_position: x: {}, z:{}", player.position.x, player.position.z);
    //println!("current position: x: {}, z:{}", current_x, current_z);

    //check z top
    let index_x = current_x;
    let index_z = f32::floor(player.position.z);
    if mini_map[index_z as usize][index_x as usize] {
        z = config.vertical_offset as f32 + current_z*config.cell_height + config.cell_height/2.0;
    }

    //z bottom
    let index_x = current_x;
    let index_z = f32::ceil(player.position.z);
    if mini_map[index_z as usize][index_x as usize] {
         z = config.vertical_offset as f32 + (index_z - 1.0)*config.cell_height + config.cell_height/2.0;
    }

    //check x left
    let index_x = f32::floor(player.position.x);
    let index_z = current_z;
    if mini_map[index_z as usize][index_x as usize] {
        x = config.horizontal_offset as f32 + (index_x + 1.0)*config.cell_width + config.cell_width/2.0;
    }

    //check x right
    let index_x = f32::floor(player.position.x) + 1.0;
    let index_z = current_z;
    if mini_map[index_z as usize][index_x as usize] {
        x = config.horizontal_offset as f32 + (index_x - 1.0)*config.cell_width + config.cell_width/2.0;
    }
    */
    
  
    let size = vec2(image_size, image_size);
    //find angle
    //orientation
    let o = vec3(player.orientation.x, player.orientation.y, player.orientation.z);
    //same direction of arrow as on png
    let n = vec3(0.0, 0.0, -1.0);
    let cos_theta = o.dot(n);
    let mut theta = cos_theta.acos();
    //rotation 360 degrees insted of 180
    let cross =  o.cross(n);
    if cross.y < 0.0 {
        theta = 2.0*PI as f32 - theta;
    }
    // println!("cos_theta: {}", cos_theta.acos().to_degrees());
    // println!("theta: {}", theta);

    //draw_plane(center, size, Some(up_texture), color);
    draw_texture_ex(up_texture, x, z, WHITE, DrawTextureParams { dest_size: Some(size), source: None, rotation: theta, flip_x: false, flip_y: false, pivot: None });

    //draw_circle(x, z, radius, color);
    
}
fn draw_walls(mini_map: &Vec<Vec<bool>>, texture: Option<&Texture2D>, color: Color){
    for (z, line) in mini_map.into_iter().enumerate(){
        for(x, cell) in line.iter().enumerate(){
            if *cell{   
                let position = vec3(x as f32, 1.0, z as f32);
                let size = vec3(1.0, 1.0, 1.0);             
                //draw_cube_wires(vec3(x as f32, 1.0, z as f32), vec3(x as f32 + 1.0, 2.0, z as f32 + 1.0), BLACK);
                draw_cube(position, size, texture, color);
            }
           
        }
    }
}
fn draw_wall_along_x(position: &Vec3, len: u32, texture: &Texture2D){   
   
    let size = vec3(len as f32, 1.0, 1.0);  
    let mut pos = *position;
    pos.x += 0.5*(len as f32 - 1.0);
    if pos.x < 0.0 {
        pos.x = 0.0;
    }
    draw_cube(pos, size, Some(&texture), WHITE);       
    //draw_cube_wires(pos, size, BLACK);   
}
fn draw_wall_along_z(position: &Vec3, len: u32, texture: &Texture2D){
   
    let size = vec3(1.0, 1.0, len as f32);  
    let mut pos = *position;
    pos.z += 0.5*(len as f32 - 1.0);
    if pos.z < 0.0 {
        pos.z = 0.0;
    }
    draw_cube(pos, size, Some(&texture), WHITE);       
    //draw_cube_wires(pos, size, BLACK);  
}
fn handle_wall_collisions(mini_map: &Vec<Vec<bool>>, prev_pos: Vec3, position: &mut Vec3, gap: f32){
    

    let mut pos = position.clone();
    pos.z = prev_pos.z;
    let points = [
        (pos.x + 0.5 + gap,  pos.z + 0.5 + gap),
        (pos.x + 0.5 - gap,  pos.z + 0.5 - gap),
        (pos.x + 0.5 + gap,  pos.z + 0.5 - gap),
        (pos.x + 0.5 - gap,  pos.z + 0.5 + gap)
    ];
    let floors = points.map(|item| (f32::floor(item.0), f32::floor(item.1)));
    for floor in floors{
        if mini_map[floor.1 as usize][floor.0 as usize] {
            position.x = prev_pos.x;
            break; 
        }
    }

    let mut pos = position.clone();
    pos.x = prev_pos.x;

    let points = [
        (pos.x + 0.5 + gap,  pos.z + 0.5 + gap),
        (pos.x + 0.5 - gap,  pos.z + 0.5 - gap),
        (pos.x + 0.5 + gap,  pos.z + 0.5 - gap),
        (pos.x + 0.5 - gap,  pos.z + 0.5 + gap)
    ];
    let floors = points.map(|item| (f32::floor(item.0), f32::floor(item.1)));
    for floor in floors{
        if mini_map[floor.1 as usize][floor.0 as usize] {
            position.z = prev_pos.z;
            break; 
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

