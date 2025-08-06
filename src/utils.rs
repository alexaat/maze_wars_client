use std::fs;
use macroquad::prelude::{vec3, Vec3};
use std::{f64::consts::PI};

//read content of the file into string
pub fn read_file(file_path: &str) -> Result<std::string::String, std::io::Error> {
    fs::read_to_string(file_path)
}


/*
    check that map
  - has all lines with same size,
  - comtains minimum 3 lanes,
  - contains at least one empty cell
*/ 
pub fn is_map_valid(content: &String) -> bool{
    let mut len: usize = 0;
    let mut num_of_lines: u32 = 0;
    let mut num_of_empty: u32 = 0;
    for line in content.lines(){
        for ch in line.chars(){
            if ch == ' ' {
                num_of_empty+=1;
            }
        }

        num_of_lines += 1;
        if len == 0{
            len = line.len();
        } else {
            if len != line.len(){
                return false;
            }
        }
    }
    if len == 0 {
        return false;
    }
    if num_of_lines < 3 {
        return false;
    }
    if num_of_empty == 0 {
        return false;
    }   

    true    
    
}

//converts srting conttents into vector of vectots whre "true" represents a wall and "false" represents empty cell
pub fn map_to_slice(content: &String) -> Vec<Vec<bool>>{
    let mut map = vec![];
    for line in content.lines(){
        let mut l = vec![];
        for ch in line.chars(){
            if ch == ' '{
                l.push(false);
            }else {
                l.push(true);
            }
        }
        map.push(l);
    }
    map
}

pub fn generate_up_to(num: usize) -> usize{
    rand::random_range(0..num)
}

pub fn orientaion_to_degrees(orientation: Vec3) -> f32{
    //orientation
    let o = vec3(
        orientation.x,
        orientation.y,
        orientation.z,
    );
    //same direction of arrow as on png
    let n = vec3(0.0, 0.0, -1.0);
    let cos_theta = o.dot(n);
    let mut theta = cos_theta.acos();
    //rotation 360 degrees insted of 180
    let cross = o.cross(n);
    if cross.y < 0.0 {
        theta = 2.0 * PI as f32 - theta;
    }
    theta
}


pub fn is_valid_ip_char(c: char) -> bool {
    if c >= '0' && c <= '9' {
        return true;
    }
    if c == '.' || c == ':' {
        return true;
    }
    false
}

pub fn is_valid_name_char(c: char) -> bool {
    c >= ' ' && c <= '~'
}
