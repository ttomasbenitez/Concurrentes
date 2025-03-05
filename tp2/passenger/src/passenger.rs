use actix::prelude::*;
use driver::{
    format_msg,
    messages::{driver_messages::*, internal_driver_messages::SetNewWriter, passenger_messages::*},
    DRIVER_PORT, MAX_DRIVERS,
};
use payments::{
    messages::{MakePayment, PaymentMsg, ValidatePayment},
    PAYMENTS_PORT,
};
use rand::Rng;
use std::io::BufRead;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf},
    net::TcpStream,
};

/// Represents a passenger that whants to make a trip.
/// - `id` - The passenger's unique identifier.
/// - `location` - The passenger's current location.
/// - `destination` - The passenger's destination.
/// - `writer` - The writer half of the TCP stream to communicate with the driver.
#[derive(Debug)]
pub struct Passenger {
    pub id: u16,
    pub location: (u8, u8),
    pub destination: (u8, u8),
    pub writer: Option<WriteHalf<TcpStream>>,
}

/// Implementation of the `Actor` trait for `Passenger`.
impl Actor for Passenger {
    type Context = Context<Self>;
}

impl Passenger {
    /// Creates a new `Passenger` instance.
    ///
    /// # Arguments
    ///
    /// * `id` - The passenger's unique identifier.
    /// * `location` - The passenger's current location.
    /// * `destination` - The passenger's destination.
    ///
    /// # Returns
    ///
    /// A new `Passenger` instance.
    pub fn new(id: u16, location: (u8, u8), destination: (u8, u8)) -> Self {
        Self {
            id,
            location,
            destination,
            writer: None,
        }
    }

    /// Creates a new passenger and attempts to connect to a driver to request a trip.
    ///
    /// # Arguments
    ///
    /// * `id` - The passenger's unique identifier.
    /// * `location` - The passenger's current location.
    /// * `destination` - The passenger's destination.
    /// * `card_number` - The passenger's card number for payment.
    ///
    /// # Returns
    ///
    /// A `Future` that resolves to `()` when the passenger has completed the trip.
    /// If the passenger fails to connect to a driver, an error message is returned.
    pub async fn create(
        id: u16,
        location: (u8, u8),
        destination: (u8, u8),
        card_number: u64,
    ) -> Result<(), String> {
        let mut passenger = Self::new(id, location, destination);

        Self::validate_payment(id, card_number).await?;

        let stream = try_connect(id, None).await;

        if let Ok(mut stream) = stream {
            let msg = format_msg(DriverMsg::TripRequest(TripRequest {
                start: location,
                end: destination,
            }));
            if stream.write_all(&msg).await.is_err() {
                return Err(format!(
                    "[PASSENGER {} ERROR] Could not send message to driver.",
                    id
                ));
            }
            let (reader, writer) = tokio::io::split(stream);
            passenger.writer = Some(writer);
            let passenger_addr = passenger.start();

            handle_recive(passenger_addr, reader, id).await;

            return Ok(());
        }
        Err(format!(
            "[PASSENGER {} ERROR] Could not connect to any driver.",
            id
        ))
    }

    /// Validates the payment for a passenger.
    ///
    /// # Arguments
    ///
    /// * `id` - The passenger's unique identifier.
    /// * `card_number` - The passenger's card number for payment.
    ///
    /// # Returns
    ///
    /// `Ok(())` if the payment is successfully validated, or an error message if the payment is declined.
    async fn validate_payment(id: u16, card_number: u64) -> Result<(), String> {
        let mut payments = match TcpStream::connect(format!("127.0.0.1:{PAYMENTS_PORT}")).await {
            Ok(stream) => stream,
            Err(_) => {
                return Err(format!(
                    "[PASSENGER {} ERROR] Could not connect to payments service",
                    id
                ))
            }
        };

        let msg = format_msg(PaymentMsg::ValidatePayment(ValidatePayment {
            passenger_id: id,
            card_number,
        }));
        if payments.write_all(&msg).await.is_err() {
            return Err(format!(
                "[PASSENGER {} ERROR] Could not send message to payments service",
                id
            ));
        }

        let mut bytes = [0; 2048];
        let n = match payments.read(&mut bytes).await {
            Ok(size) => size,
            Err(_) => {
                return Err(format!(
                    "[PASSENGER {} ERROR] Failed to read response from payments service",
                    id
                ))
            }
        };

        let response: PaymentMsg = match serde_json::from_slice(&bytes[..n]) {
            Ok(resp) => resp,
            Err(_) => {
                return Err(format!(
                    "[PASSENGER {} ERROR] Failed to parse response from payments service",
                    id
                ))
            }
        };

        if let PaymentMsg::ValidatePaymentResponse(response) = response {
            match response.status {
                payments::messages::ValidationStatus::Success => {
                    println!("[PASSENGER {}] Payment validated!", id);
                    Ok(())
                }
                payments::messages::ValidationStatus::Failure => {
                    Err(format!("[PASSENGER {} ERROR] Payment declined", id))
                }
            }
        } else {
            Err(format!(
                "[PASSENGER {} ERROR] Invalid response from payments service",
                id
            ))
        }
    }
}

/// Handles messages received from a driver.
///
/// # Arguments
///
/// * `passenger` - The address of the passenger actor.
/// * `stream` - The reader half of the TCP stream to communicate with the driver.
/// * `id` - The passenger's unique identifier.
///
/// # Returns
///
/// A `Future` that resolves to `()` when the passenger has completed the trip.
/// If the passenger fails to connect to a driver, an error message is returned.
pub async fn handle_recive(passenger: Addr<Passenger>, stream: ReadHalf<TcpStream>, id: u16) {
    let mut bytes = [0; 2048];
    let mut current_stream = Some(stream);

    loop {
        if let Some(ref mut stream) = current_stream {
            match stream.read(&mut bytes).await {
                Ok(0) => {
                    println!("[PASSENGER {} WARNING] Connection to driver lost.", id);
                    if let Ok(new_stream) = try_connect(id, None).await {
                        let (reader, writer) = tokio::io::split(new_stream);
                        let _ = passenger.send(SetNewWriter { writer }).await;
                        current_stream = Some(reader);
                        continue;
                    } else {
                        println!("[PASSENGER {} ERROR] Could not connect to any driver.", id);
                        return;
                    }
                }
                Ok(n) => {
                    for line in bytes[..n].lines().map_while(Result::ok) {
                        match serde_json::from_str(&line) {
                            Ok(PassengerMsg::TripResponse(response)) => {
                                let mut rng = rand::thread_rng();
                                let try_again: bool = rng.gen_bool(0.7);
                                let status = response.status;
                                passenger.do_send(HandleTripResponse {
                                    res: response,
                                    try_again,
                                });
                                if !status && !try_again {
                                    return;
                                }
                            }
                            Ok(PassengerMsg::TripEnded) => {
                                println!("[PASSENGER {}] Trip ended successfully!", id);
                                make_payment(id).await.ok();
                                passenger.do_send(TripEnded { passenger_id: id });
                                return;
                            }
                            _ => eprintln!(
                                "[PASSENGER {} ERROR] Invalid message received from driver.",
                                id
                            ),
                        }
                    }
                }
                Err(_) => {
                    println!("[PASSENGER {} ERROR] Failed to read from stream.", id);
                    return;
                }
            }
        }
    }
}

/// Implementation of the `Handler` trait for `TripResponse` messages sent to `Passenger`.
impl Handler<HandleTripResponse> for Passenger {
    type Result = ();

    /// Handles a `TripResponse` message sent to the `Passenger` actor.
    ///
    /// - If the trip is accepted:
    ///   - A success message is printed to the console.
    /// - If the trip is declined:
    ///   - A decline message is printed to the console.
    ///   - The passenger may decide whether to try again:
    ///     - If trying again:
    ///       - A new trip request is sent to the driver.
    ///     - If not trying again:
    ///       - The actor is stopped.
    ///
    /// # Arguments
    ///
    /// * `msg` - The `HandleTripResponse` message to handle.
    /// * `ctx` - The actor context.
    ///
    /// # Returns
    ///
    /// `()` when the message has been handled.
    fn handle(&mut self, msg: HandleTripResponse, ctx: &mut Self::Context) -> Self::Result {
        if msg.res.status {
            println!("[PASSENGER {}] Trip accepted by driver!", self.id);
        } else {
            match msg.res.reason {
                Some(DeclineReason::NotAccepted) => {
                    println!(
                        "[PASSENGER {}] The trip was not accepted by any driver.",
                        self.id
                    );
                }
                Some(DeclineReason::DriversBusy) => {
                    println!("[PASSENGER {}] All drivers are busy.", self.id);
                }
                None => {
                    println!(
                        "[PASSENGER {}] The trip was declined by the drivers.",
                        self.id
                    );
                }
            }
            if msg.try_again {
                let mut writer = match self.writer.take() {
                    Some(writer) => writer,
                    None => return,
                };
                let start = self.location;
                let end = self.destination;
                let passenger_id = self.id;
                async move {
                    let msg = format_msg(DriverMsg::TripRequest(TripRequest { start, end }));
                    if writer.write_all(&msg).await.is_err() {
                        println!(
                            "[PASSENGER {} ERROR] Could not send message to driver.",
                            passenger_id
                        );
                    } else {
                        println!("[PASSENGER {}] Trying again...", passenger_id);
                    }
                    writer
                }
                .into_actor(self)
                .map(|res, passenger, _| {
                    passenger.writer = Some(res);
                })
                .wait(ctx);
            } else {
                println!(
                    "[PASSENGER {}] The passenger has decided not to try again.",
                    self.id
                );
                ctx.stop();
            }
        }
    }
}

/// Implementation of the `Handler` trait for `TripEnded` messages sent to `Passenger`.
impl Handler<TripEnded> for Passenger {
    type Result = ();

    /// Handles a `TripEnded` message sent to the `Passenger` actor.
    /// The actor is stopped when the trip ends.
    ///
    /// # Arguments
    ///
    /// * `msg` - The `TripEnded` message to handle.
    /// * `ctx` - The actor context.
    ///
    /// # Returns
    ///
    /// `()` when the message has been handled.
    fn handle(&mut self, _msg: TripEnded, ctx: &mut Self::Context) -> Self::Result {
        ctx.stop();
    }
}

/// Implementation of the `Handler` trait for `SetNewWriter` messages sent to `Passenger`.
impl Handler<SetNewWriter> for Passenger {
    type Result = ();

    /// Handles a `SetNewWriter` message sent to the `Passenger` actor.
    /// The writer half of the TCP stream is updated with the new writer.
    ///
    /// # Arguments
    ///
    /// * `new_writer` - The `SetNewWriter` message to handle containing the new writer.
    /// * `ctx` - The actor context.
    ///
    /// # Returns
    ///
    /// `()` when the message has been handled.
    fn handle(&mut self, new_writer: SetNewWriter, _: &mut Self::Context) -> Self::Result {
        self.writer = Some(new_writer.writer);
    }
}

/// Attempts to connect to a driver to request a trip.
///
/// # Arguments
///
/// * `id` - The passenger's unique identifier.
/// * `location` - The passenger's current location.
/// * `destination` - The passenger's destination.
///
/// # Returns
///
/// A `Result` containing the TCP stream to communicate with the driver if the connection is successful,
/// or an error message if the connection fails.
async fn try_connect(id: u16, leader_driver: Option<u16>) -> Result<TcpStream, ()> {
    let mut stack = vec![leader_driver.unwrap_or(0)];
    while let Some(leader_driver) = stack.pop() {
        for driver_id in leader_driver..MAX_DRIVERS {
            let ip = format!("127.0.0.1:{}", DRIVER_PORT + driver_id);

            let mut stream = match TcpStream::connect(ip).await {
                Ok(stream) => stream,
                Err(_) => continue,
            };

            let msg = format_msg(DriverMsg::Connect(Connect {
                from: ConnectionType::Passenger,
                id,
                coordinator_id: None,
            }));

            if stream.write_all(&msg).await.is_ok() {
                let mut bytes = [0; 2048];
                let n = match stream.read(&mut bytes).await {
                    Ok(size) => size,
                    Err(_) => return Err(()),
                };

                let response: PassengerMsg = match serde_json::from_slice(&bytes[..n]) {
                    Ok(resp) => resp,
                    Err(_) => return Err(()),
                };

                if let PassengerMsg::ConnectRes(response) = response {
                    if !response.status {
                        if let Some(leader_id) = response.leader_id {
                            stack.push(leader_id);
                        } else {
                            stack.push(leader_driver + 1);
                        }
                        break;
                    }
                }

                println!("[PASSENGER {}] Connected to driver {}.", id, driver_id);
                return Ok(stream);
            }
        }
    }
    Err(())
}

/// Sends a payment request to the payments service to make the payment.
///
/// # Arguments
///
/// * `id` - The passenger's unique identifier.
///
/// # Returns
///
/// `Ok(())` if the payment is successfully made, or an error message if the payment fails.
async fn make_payment(id: u16) -> Result<(), String> {
    let mut payments = match TcpStream::connect(format!("127.0.0.1:{PAYMENTS_PORT}")).await {
        Ok(stream) => stream,
        Err(_) => {
            return Err(format!(
                "[PASSENGER {} ERROR] Could not connect to payments service",
                id
            ))
        }
    };
    let msg = format_msg(PaymentMsg::MakePayment(MakePayment { passenger_id: id }));
    if payments.write_all(&msg).await.is_err() {
        return Err(format!(
            "[PASSENGER {} ERROR] Could not send message to payments service",
            id
        ));
    }
    Ok(())
}
