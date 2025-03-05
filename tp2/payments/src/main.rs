use payments::{payments::Payments, PAYMENTS_PORT};

fn main() {
    let payments = Payments::new(PAYMENTS_PORT);
    match payments.start_listening() {
        Ok(_) => println!("Payments service started successfully"),
        Err(e) => eprintln!("Error starting payments service: {:?}", e),
    }
}
