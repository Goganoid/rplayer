use player::App;
use std::path::PathBuf;

fn help(){
    println!("Usage:\n player [path]");
    println!("Shortcuts:");
    println!(" Ctrl + Left/Right arrow - Move timestamp");
    println!(" Left/Right arrow - Set previous/next track");
    println!(" Alt + Left/Right arrow - Skip 5 tracks and set track");
    println!(" Space - Toggle pause");
    println!(" Up/Down arrow - Control volume");
    println!(" S - shuffle");
    println!(" Esc - close player");
}

fn main() {

    let args:Vec<String>= std::env::args().collect();
    match args.len(){
        1 => help(),
        2 =>{
            let mut app = App::new(PathBuf::from(args[1].clone()));
            app.run().unwrap();
        }
        _ =>{
            println!("Too many arguments")
        }
    }


}
