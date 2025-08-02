use std::fs;
use rand::Rng;

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