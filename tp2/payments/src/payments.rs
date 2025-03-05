use crate::{
    format_msg,
    messages::{PaymentMsg, ValidatePaymentResponse, ValidationStatus},
};
use std::{
    io::{self, Read, Write},
    net::TcpListener,
    thread,
};

/// Represents the Payments service, which handles payment validation
/// and payment processing.
pub struct Payments {
    port: u16,
}

impl Payments {
    /// Creates a new instance of the Payments service.
    ///
    /// # Arguments
    ///
    /// * `port` - The port number the service will listen on.
    ///
    /// # Returns
    ///
    /// A new `Payments` instance.
    pub fn new(port: u16) -> Self {
        Self { port }
    }

    /// Starts the Payments service and listens for incoming connections.
    ///
    /// # Returns
    ///
    /// `Ok(())` if the service starts successfully, or an `io::Error`
    /// if the server fails to bind to the specified port.
    pub fn start_listening(&self) -> io::Result<()> {
        let listener = TcpListener::bind(format!("127.0.0.1:{}", self.port))?;
        println!("Payments service listening on port {}", self.port);
        for stream in listener.incoming().flatten() {
            thread::spawn(move || Self::handle_messages(stream));
        }
        Ok(())
    }

    /// Handles messages received from a single client connection.
    ///
    /// This function reads and processes messages from the client. It can validate
    /// payments or acknowledge payment actions.
    ///
    /// # Arguments
    ///
    /// * `stream` - The client connection implementing `Read` and `Write`.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the message handling completes, or an `io::Error` if there is an issue
    /// with the client connection.
    fn handle_messages<T: Read + Write>(mut stream: T) -> io::Result<()> {
        let mut buffer = [0; 2048];
        let n = stream.read(&mut buffer)?;
        if n == 0 {
            return Ok(());
        }
        match serde_json::from_slice(&buffer[..n]) {
            Ok(PaymentMsg::ValidatePayment(passenger)) => {
                let is_valid = passenger.card_number % 2 == 0;
                let message = format_msg(PaymentMsg::ValidatePaymentResponse(
                    ValidatePaymentResponse {
                        status: if is_valid {
                            ValidationStatus::Success
                        } else {
                            ValidationStatus::Failure
                        },
                    },
                ));
                if is_valid {
                    println!(
                        "Payment validation SUCCESS for passenger: {:}",
                        passenger.passenger_id
                    );
                    stream.write_all(&message)?;
                } else {
                    println!(
                        "Payment validation FAILD for passenger: {:}",
                        passenger.passenger_id
                    );
                    stream.write_all(&message)?;
                }
            }
            Ok(PaymentMsg::MakePayment(passenger)) => {
                println!("Payment made from passenger: {:}", passenger.passenger_id);
            }
            Err(e) => {
                eprintln!("Error parsing message: {}", e);
            }
            _ => {
                eprintln!("Invalid message received: {:?}", &buffer[..n]);
            }
        }
        stream.flush()?;
        Ok(())
    }
}
