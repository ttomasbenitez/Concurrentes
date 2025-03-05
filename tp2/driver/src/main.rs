use driver::{driver::Driver, MAX_DRIVERS};
use std::env;

#[actix_rt::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 4 {
        eprintln!("Usage: cargo run <id> <x_position> <y_position>");
        return;
    }
    let id: u16 = args[1].parse().expect("Id must be a number");
    let x_position: u8 = args[2].parse().expect("x_position must be a number");
    let y_position: u8 = args[3].parse().expect("y_position must be a number");

    if id > MAX_DRIVERS - 1 {
        eprintln!("ID must be less than {}", MAX_DRIVERS);
        return;
    }

    let driver_handle = Driver::start(id, x_position, y_position).await;
    match driver_handle {
        Ok(handle) => match handle.await {
            Ok(_) => println!("Driver stopped"),
            Err(e) => eprintln!("{:?}", e),
        },
        Err(e) => eprintln!("Error starting driver: {:?}", e),
    }
}
