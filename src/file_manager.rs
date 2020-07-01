use std::path::{PathBuf, Path};
use glob::glob;
use std::cell::{RefCell};
use rand::seq::SliceRandom;
use mp3_metadata::MP3Metadata;

pub struct FileManager{
    pub file_paths: Vec<PathBuf>,
    rng:rand::rngs::ThreadRng,
    indexes:Vec<usize>,
    cur: RefCell<usize>,
    size: usize,
    is_shuffled:bool,
}

impl FileManager{
    pub fn new(dir:&Path) -> Result<FileManager,()>{
        if !dir.is_dir() { return Err(()) }
        let mut dir_str = String::from(dir.to_str().unwrap());
        if !dir.ends_with("/") {dir_str+="/"}
        let format_pattern = dir_str + "*.";
        let mut file_paths = Vec::new();
        for entry in glob(format!("{}{}",format_pattern.as_str(),"mp3").as_str()).expect("Failed to read glob pattern"){
            match entry {
                Ok(path) =>file_paths.push(path),
                Err(e) => println!("{:?}", e),
            }
        }
        let len = file_paths.len();
        let indexes:Vec<usize> = (0..len).collect();
        if len!=0{
            Ok(FileManager{file_paths,indexes,cur:RefCell::new(0),size:len,rng:rand::thread_rng(),is_shuffled:false})
        }
        else {
            println!("\rDirectory is empty");
            Err(())
        }

    }
    pub fn is_shuffled(&self) ->bool{
        self.is_shuffled
    }
    pub fn current_index(&self) -> usize{
        self.indexes[*self.cur.borrow()]
    }
    pub fn toggle_shuffle(&mut self){
        self.is_shuffled = !self.is_shuffled;
        self.make_shuffled(self.is_shuffled);
    }
    pub fn make_shuffled(&mut self,shuffle:bool){
        if shuffle{
            self.is_shuffled = true;
            let cur_index = *self.cur.borrow();
            self.indexes[0..cur_index].shuffle(&mut self.rng);
            if *self.cur.borrow()!= self.size-1{
                self.indexes[cur_index+1..].shuffle(&mut self.rng);
            }
        }
        else{
            self.is_shuffled = false;
            let index = self.indexes[*self.cur.borrow()];
            *self.cur.borrow_mut() = index;
            self.indexes = (0..self.size).collect();
        }
    }
    pub fn get_paths(&self,start:usize, end:usize) -> Vec<PathBuf>{
        let mut result = Vec::new();
        for index in self.indexes[start..end].iter(){
            result.push(self.file_paths[*index].clone());
        }
        result
    }
    pub fn get_metadata(&mut self,filename:&Path) -> MP3Metadata{

        mp3_metadata::read_from_file(filename).unwrap()
    }
    pub fn next(&self) -> Option<PathBuf>{
        let mut result = None;
        if *self.cur.try_borrow().unwrap()<self.size-1 {
            *self.cur.borrow_mut()+=1;
            result = Some(self.get_current());

        }
        result

    }
    fn get_index(&self,index:usize) -> usize{
        self.indexes[index]
    }
    pub fn get_current(&self) -> PathBuf{
        self.file_paths[self.get_index(*self.cur.try_borrow().unwrap())].clone()
    }
    pub fn prev(&self) -> Option<PathBuf>{
        let mut result = None;
        if *self.cur.try_borrow().unwrap()>0 {
            *self.cur.borrow_mut()-=1;
            result = Some(self.get_current());
        }
        result

    }
    pub fn set_index(&self, new_index:usize){
        *self.cur.borrow_mut() = new_index;
    }
    pub fn tracks_left(&self) -> usize{
        self.size - 1 - *self.cur.borrow()
    }
}
